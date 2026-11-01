// Sums and conversions, answered as they are typed: "1284 * 0.18" shows 231.12, "3.5 km in miles" 2.1748 mi. Enter
// copies the answer (GNOME's own result copies a result's clipboardText) and closes the search. The arithmetic is
// calc.js's, which reads the text itself and never runs it. Nothing is held between searches.

import GLib from 'gi://GLib';

import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import {gettext as _} from 'resource:///org/gnome/shell/extensions/extension.js';

import {evaluate, localeTag, numericLocale} from './calc.js';

export class AnswersProvider {
    constructor() {
        this.id = 'amethystora-answers';
        this.isRemoteProvider = false;
        this.canLaunchSearch = false;
        this._answer = null;
    }

    // The session's own numbers (GNOME Shell's environment carries the session's locale variables), and the words that
    // join two units in its language ("in|to", translated)
    _options() {
        const env = Object.fromEntries(['LC_ALL', 'LC_NUMERIC', 'LANG'].map(name => [name, GLib.getenv(name)]));
        const locale = localeTag(numericLocale(env));
        const connectors = _('in|to').split('|').map(word => word.trim().toLowerCase()).filter(Boolean);
        return {locale, connectors};
    }

    // GNOME splits the text at every space, the no-break ones a number is grouped with included: the field's own text is
    // read instead, when it is what the words came from
    _text(terms) {
        const typed = Main.overview.searchEntry?.get_text()?.trim() ?? '';
        return typed.split(/\s+/u).join(' ') === terms.join(' ') ? typed : terms.join(' ');
    }

    getInitialResultSet(terms) {
        this._answer = evaluate(this._text(terms), this._options());
        return Promise.resolve(this._answer ? ['answer'] : []);
    }

    getSubsearchResultSet(previous, terms) {
        return this.getInitialResultSet(terms);
    }

    getResultMetas(ids) {
        const answer = this._answer;
        return Promise.resolve(ids.map(id => answer && id === 'answer'
            ? {
                id, name: answer.display, clipboardText: answer.copy, expression: answer.expression, answer: true,
                hint: _('Enter copies it'),
            }
            : {id, name: id}));
    }

    filterResults(ids, max) {
        return ids.slice(0, max);
    }

    // GNOME's result copies the answer itself, from its clipboardText
    activateResult() {}
}
