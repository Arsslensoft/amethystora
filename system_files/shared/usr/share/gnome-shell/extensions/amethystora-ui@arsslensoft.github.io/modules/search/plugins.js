// What plugins that contribute `search` answer in Super+Space (R7-07). The answer is what the plugin last printed, shown when the text
// starts with its prefix (or, with none, when its first word begins the plugin's name). What is typed never goes to a plugin: a sandbox
// is asked nothing here, so search words stay in the shell; the plugin's output is as fresh as its own interval. Enter runs the
// plugin's `click` action. Texts are drawn as plain text, whatever the plugin wrote.

import {gettext as _} from 'resource:///org/gnome/shell/extensions/extension.js';

export class PluginsProvider {
    constructor(ext) {
        this._ext = ext;
        this.id = 'amethystora-plugins';
        this.isRemoteProvider = false;
        this.canLaunchSearch = false;
    }

    enable() {
        this._unlisten = this._plugins?.onChanged(() => this.changed?.()) ?? null;
    }

    destroy() {
        this._unlisten?.();
        this._unlisten = null;
    }

    get _plugins() {
        return this._ext.modules?.get('plugins') ?? null;
    }

    getInitialResultSet(terms) {
        this._text = terms.join(' ');
        return Promise.resolve((this._plugins?.searchAnswers(terms.join(' ')) ?? []).map(a => a.id));
    }

    getSubsearchResultSet(previous, terms) {
        return this.getInitialResultSet(terms);
    }

    getResultMetas(ids) {
        const answers = new Map((this._plugins?.searchAnswers(this._text ?? '') ?? []).map(a => [a.id, a]));
        return Promise.resolve(ids.map(id => {
            const a = answers.get(id) ?? this._plugins?.answerOf(id);
            return a ? {
                id, name: a.text ? `${a.name}: ${a.text}` : a.name, symbol: a.icon || 'applications-system-symbolic',
                hint: a.action ? _('Enter') : '', note: a.draft ? _('Draft') : '',
            } : {id, name: id};
        }));
    }

    filterResults(ids, max) {
        return ids.slice(0, max);
    }

    activateResult(id) {
        this._plugins?.answerOf(id)?.run?.();
    }
}

