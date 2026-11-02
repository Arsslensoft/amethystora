// "tea 4 min" in search: a row that starts a timer called Tea with Enter (R7-22). The search reads the words (timer.js),
// and nothing happens until Enter. The live pill runs it: while the pill is off, or two run already, the row says so
// rather than pretend. The words stay in memory.

import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import {gettext as _, ngettext} from 'resource:///org/gnome/shell/extensions/extension.js';

import {readTimer} from './timer.js';

// "4 minutes", "1 hour 30 minutes", "90 seconds"
function length(seconds) {
    if (seconds < 60)
        return ngettext('%d second', '%d seconds', seconds).format(seconds);
    const minutes = Math.floor(seconds / 60);
    const words = ngettext('%d minute', '%d minutes', minutes).format(minutes);
    const rest = seconds % 60;
    return rest ? `${words} ${ngettext('%d second', '%d seconds', rest).format(rest)}` : words;
}

export class TimerProvider {
    constructor(ext) {
        this.id = 'amethystora-timer';
        this.isRemoteProvider = false;
        this.canLaunchSearch = false;
        this._ext = ext;
        this._row = null;
    }

    enable() {}

    destroy() {
        this._row = null;
    }

    _text(terms) {
        const typed = Main.overview.searchEntry?.get_text()?.trim() ?? '';
        return typed.split(/\s+/u).join(' ') === terms.join(' ') ? typed : terms.join(' ');
    }

    get _pill() {
        return this._ext.modules?.get('pill') ?? null;
    }

    getInitialResultSet(terms) {
        const timer = readTimer(this._text(terms));
        this._row = timer && this._pill?.canStartTimer() ? timer : null;
        return Promise.resolve(this._row ? ['timer'] : []);
    }

    getSubsearchResultSet(previous, terms) {
        return this.getInitialResultSet(terms);
    }

    getResultMetas(ids) {
        const timer = this._row;
        return Promise.resolve(ids.map(id => ({
            id,
            name: timer?.name ? _('Start a timer called %s').format(timer.name) : _('Start a timer'),
            symbol: 'alarm-symbolic',
            description: timer ? length(timer.seconds) : '',
            hint: _('Enter'),
        })));
    }

    filterResults(ids, max) {
        return ids.slice(0, max);
    }

    activateResult() {
        if (this._row)
            this._pill?.startTimer(this._row.name, this._row.seconds);
    }

    // For the suites: the row a text would show
    answerFor(text) {
        return readTimer(text);
    }
}
