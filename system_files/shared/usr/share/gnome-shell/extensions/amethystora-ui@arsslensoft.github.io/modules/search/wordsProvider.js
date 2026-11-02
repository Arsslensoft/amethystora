// It speaks your old words (plans/ui-ideas/r7/spec-find.md, R7-49): the row that takes a word someone already knows, such
// as "recycle bin", to the thing here. The table is in words.js. It is read once, in the session's language, at the
// first search; nothing runs until then and nothing runs after. Only what this machine has is offered: an app that is not
// installed is passed over and never installed from here.

import GLib from 'gi://GLib';
import Gio from 'gi://Gio';
import Shell from 'gi://Shell';
import St from 'gi://St';

import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import {gettext as _} from 'resource:///org/gnome/shell/extensions/extension.js';

import {appNamedLike, compileWords, localeTag, matchWords, wordsTable} from './words.js';
import {finished} from './run.js';

const IN_TERMINAL = '/usr/libexec/amethystora-in-terminal';

export class WordsProvider {
    constructor() {
        this.id = 'amethystora-words';
        this.isRemoteProvider = false;
        this.canLaunchSearch = false;
        this._compiled = null;
        this._byId = new Map();
    }

    destroy() {
        this._gone = true;
        this._afterHidden?.();
        this._compiled = null;
        this._byId = new Map();
    }

    _table() {
        if (!this._compiled) {
            this._locale = localeTag(GLib.get_language_names_with_category('LC_MESSAGES')[0]);
            this._compiled = compileWords(wordsTable(text => text), wordsTable(_), this._locale);
            this._byId = new Map(this._compiled.map(entry => [entry.id, entry]));
        }
        return this._compiled;
    }

    // The first of an entry's apps that this machine has
    _appOf(entry) {
        const system = Shell.AppSystem.get_default();
        for (const id of entry.apps ?? []) {
            const app = system.lookup_app(id);
            if (app)
                return app;
        }
        return null;
    }

    getInitialResultSet(terms) {
        const table = this._table();
        let found = matchWords(table, terms, this._locale).filter(entry => !entry.apps || this._appOf(entry));
        if (found.length) {
            // An installed app named like what was typed keeps Enter, unless it is the entry's own
            const installed = Shell.AppSystem.get_default().get_installed().map(info => [info.get_name() ?? '', info.get_id()]);
            found = found.filter(entry => !appNamedLike(installed, terms, this._locale, entry.apps ?? []));
        }
        return Promise.resolve(found.map(entry => entry.id));
    }

    getSubsearchResultSet(previous, terms) {
        return this.getInitialResultSet(terms);
    }

    getResultMetas(ids) {
        return Promise.resolve(ids.map(id => {
            const entry = this._byId.get(id);
            if (!entry)
                return {id, name: id};
            const hint = entry.terminal ? _('Enter opens a terminal') : _('Enter opens');
            const app = this._appOf(entry);
            if (app) {
                return {
                    id, name: app.get_name(), description: entry.note, hint,
                    createIcon: size => app.create_icon_texture(size),
                };
            }
            return {
                id, name: entry.title, description: entry.note, hint,
                createIcon: size => new St.Icon({gicon: Gio.ThemedIcon.new(entry.icon), icon_size: size}),
            };
        }));
    }

    filterResults(ids, max) {
        return ids.slice(0, max);
    }

    // After the search has closed, or its grab would be in the tool's way. From the overview, after the overview has left
    // the screen, or the picture to cut from would be the overview
    _openScreenshot() {
        const open = () => {
            if (!this._gone)
                Main.screenshotUI.open();
        };
        if (!Main.overview.visible) {
            GLib.idle_add(GLib.PRIORITY_DEFAULT, () => {
                open();
                return GLib.SOURCE_REMOVE;
            });
            return;
        }
        this._afterHidden?.();
        const handler = Main.overview.connect('hidden', () => {
            this._afterHidden();
            open();
        });
        this._afterHidden = () => {
            Main.overview.disconnect(handler);
            this._afterHidden = null;
        };
    }

    activateResult(id) {
        const entry = this._byId.get(id);
        if (!entry)
            return;
        if (entry.apps) {
            this._appOf(entry)?.activate();
        } else if (entry.uri) {
            Gio.AppInfo.launch_default_for_uri_async(entry.uri, global.create_app_launch_context(0, -1), null, (_source, result) => {
                try {
                    Gio.AppInfo.launch_default_for_uri_finish(result);
                } catch (e) {
                    console.warn(`Amethystora UI: the search could not open ${entry.id}: ${e.message}`);
                }
            });
        } else if (entry.manual) {
            finished(['amethystora-manual', entry.manual]);
        } else if (entry.argv) {
            finished(entry.terminal ? [IN_TERMINAL, '--', ...entry.argv] : entry.argv);
        } else if (entry.shell === 'screenshot') {
            this._openScreenshot();
        }
    }
}
