// The pill of your own (R7-01): the `extra` activity, after everything the system shows and before quiet and a focus profile's mark
// (pill/activities.js ORDER). One activity stands for all the plugins and scripts that want the pill: the one that changed last
// is in the pill and the rest are rows of its card, so that a call, a recording, a link or the battery's question is never hidden
// by one of them. Its plate is always '': nothing here can wear a system look. The shell draws the plugin's name, what it may reach
// and its Pause below whatever the plugin printed. A countdown (`until`) is counted here, one tick a second only while it is on
// screen, so that it costs no process.

import Clutter from 'gi://Clutter';
import GLib from 'gi://GLib';
import St from 'gi://St';

import {gettext as _} from 'resource:///org/gnome/shell/extensions/extension.js';

import {buttonRow, cardButton, retire} from '../pill/clock.js';
import {clock, plain, spoken} from '../pill/text.js';
import {elements, label, reachWords} from './card.js';
import {reach} from './clamp.js';

const OTHERS = 3;

export class ExtraMark {
    // items: [{key, kind: 'plugin' | 'draft' | 'script', name, output, plugin}] with the latest change first; ui: the module's controls
    constructor(items, ui) {
        this.id = 'extra';
        this.plate = '';
        this._items = items;
        this._ui = ui;
        this._timer = 0;
        this._showing = false;
    }

    get _top() {
        return this._items[0];
    }

    get progress() {
        return this._top.output?.progress ?? null;
    }

    get short() {
        return this._words(true);
    }

    _countdown() {
        const until = this._top.output?.until;
        return until ? Math.max(0, until - Date.now() / 1000) : null;
    }

    // The plugin's name always comes first, beside its words: whatever it prints, it is drawn as its own and never as the system's. The bubble has room
    // for the name alone
    _words(brief = false) {
        const top = this._top;
        const hidden = this._ui.hidden(top);
        const base = hidden ? _('Private') : (top.output?.text ?? '');
        const left = hidden ? null : this._countdown();
        const body = left === null ? base : [base, clock(left)].filter(Boolean).join(' · ');
        const name = top.kind === 'draft' ? `${_('Draft')} ${top.name}` : top.name;
        return brief ? name : [name, body].filter(Boolean).join(' · ');
    }

    get text() {
        return this._words();
    }

    get accessible() {
        const left = this._countdown();
        return `${this._top.name}: ${this._top.output?.text ?? ''}${left === null ? '' : `, ${spoken(left)}`}`;
    }

    get row() {
        return {icon: this._top.output?.icon || 'applications-system-symbolic', text: this.text};
    }

    // New items; whether the words in the pill changed
    set(items) {
        const before = this.text;
        this._items = items;
        if (this._label)
            this._label.text = this.text;
        if (this._icon)
            this._icon.icon_name = this._top.output?.icon || 'applications-system-symbolic';
        this._tick();
        return this.text !== before;
    }

    pill() {
        if (!this._content) {
            this._content = new St.BoxLayout({style_class: 'amethystora-pill-content amethystora-pill-extra'});
            this._icon = new St.Icon({icon_name: this._top.output?.icon || 'applications-system-symbolic', icon_size: 14, y_align: Clutter.ActorAlign.CENTER});
            this._label = new St.Label({text: this.text, y_align: Clutter.ActorAlign.CENTER});
            this._label.clutter_text.use_markup = false;
            this._content.add_child(this._icon);
            this._content.add_child(this._label);
        }
        return this._content;
    }

    // A countdown runs a tick each second while it is in the pill or the bubble, and never otherwise
    showing(on) {
        this._showing = on;
        this._tick();
    }

    _tick() {
        const want = this._showing && this._countdown() !== null;
        if (want && !this._timer) {
            this._timer = GLib.timeout_add_seconds(GLib.PRIORITY_DEFAULT, 1, () => {
                if (this._label)
                    this._label.text = this.text;
                this._ui.changed();
                if (this._countdown() === 0 && this._timer) {
                    this._timer = 0;
                    return GLib.SOURCE_REMOVE;
                }
                return GLib.SOURCE_CONTINUE;
            });
        } else if (!want && this._timer) {
            GLib.source_remove(this._timer);
            this._timer = 0;
        }
    }

    card(api) {
        const top = this._top;
        const body = new St.BoxLayout({style_class: 'amethystora-pill-card-body amethystora-plugin-card', orientation: Clutter.Orientation.VERTICAL});
        body.add_child(label(top.name, 'amethystora-pill-news-title'));
        // Whose it is, as the shell says it and not as the plugin does
        const kind = {draft: _('Draft: it runs with nothing allowed'), script: _('From a script of yours'), plugin: top.plugin?.shipped ? _('Made for Amethystora') : _('A plugin')}[top.kind];
        body.add_child(label(kind, 'amethystora-plugin-kind'));
        if (this._ui.hidden(top)) {
            body.add_child(label(_('Hidden while your screen is shared.')));
        } else {
            const output = top.output;
            if (output?.text)
                body.add_child(label(output.text));
            if (output?.card.length)
                body.add_child(elements(output.card, action => this._ui.act(top, action)));
        }
        for (const other of this._items.slice(1, 1 + OTHERS)) {
            const row = new St.BoxLayout({style_class: 'amethystora-pill-news-row'});
            row.add_child(new St.Icon({icon_name: other.output?.icon || 'applications-system-symbolic', icon_size: 14}));
            row.add_child(label(plain(`${other.name}${other.output?.text ? `: ${other.output.text}` : ''}`, 60)));
            body.add_child(row);
        }
        body.add_child(label(top.kind === 'script' ? _('Nothing about it is sandboxed: it is a program of yours.') : reachWords(reach(top.plugin?.granted ?? [])),
            'amethystora-plugin-reach'));
        const buttons = [];
        if (top.kind === 'draft') {
            buttons.push(cardButton(_('Throw away'), () => this._then(api, () => this._ui.discard(top))));
            buttons.push(cardButton(_('Change'), () => this._then(api, () => this._ui.change(top))));
            buttons.push(cardButton(_('Keep'), () => this._then(api, () => this._ui.keep(top)), {primary: true}));
        } else if (top.kind === 'script') {
            buttons.push(cardButton(_('Stop showing'), () => this._then(api, () => this._ui.stopScript(top))));
        } else if (top.plugin?.paused) {
            buttons.push(cardButton(_('Resume'), () => this._then(api, () => this._ui.resume(top)), {primary: true}));
        } else {
            buttons.push(cardButton(_('Pause'), () => this._then(api, () => this._ui.pause(top))));
        }
        body.add_child(buttonRow(buttons));
        return body;
    }

    _then(api, run) {
        api.close();
        run();
    }

    destroy() {
        if (this._timer)
            GLib.source_remove(this._timer);
        this._timer = 0;
        retire(this._content);
        this._content = this._label = this._icon = null;
    }
}
