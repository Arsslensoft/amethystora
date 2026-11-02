// The pill's mark while a flat battery's card shows (power/flat.js): the charge on the error colour's plate. A click
// opens the pill's card as it does for any other activity, which lists this one with the rest.

import Clutter from 'gi://Clutter';
import St from 'gi://St';

import {gettext as _} from 'resource:///org/gnome/shell/extensions/extension.js';

import {retire} from './clock.js';

export class FlatMark {
    constructor(percent) {
        this.id = 'flat';
        this.plate = 'flat';
        this._percent = percent;
    }

    // Whether the words changed
    setPercent(percent) {
        if (percent === this._percent)
            return false;
        this._percent = percent;
        if (this._label)
            this._label.text = this.text;
        return true;
    }

    get text() {
        return _('Battery %d%%').format(this._percent);
    }

    get accessible() {
        return this.text;
    }

    get row() {
        return {icon: 'battery-level-0-symbolic', text: this.text};
    }

    pill() {
        if (!this._content) {
            this._content = new St.BoxLayout({style_class: 'amethystora-pill-content amethystora-pill-flat'});
            this._label = new St.Label({text: this.text, y_align: Clutter.ActorAlign.CENTER});
            this._content.add_child(this._label);
        }
        return this._content;
    }

    card() {
        return new St.Label({style_class: 'amethystora-pill-card-label', text: this.text});
    }

    destroy() {
        retire(this._content);
        this._content = null;
        this._label = null;
    }
}
