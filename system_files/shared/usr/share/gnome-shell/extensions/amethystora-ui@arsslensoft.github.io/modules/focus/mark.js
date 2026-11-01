// A profile's mark in the live pill: its icon and "Work · 14:32" on the accent's plate, the pill's last activity, so
// that anything more pressing shows first. The time follows GNOME's clock, only while the mark is in the pill. Its card says what the profile does and how many banners it has held, with Turn off and a way
// to Control.

import Clutter from 'gi://Clutter';
import St from 'gi://St';

import {gettext as _, ngettext} from 'resource:///org/gnome/shell/extensions/extension.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import * as Util from 'resource:///org/gnome/shell/misc/util.js';

import {buttonRow, cardButton, retire} from '../pill/clock.js';
import {timeOfDay} from '../pill/text.js';
import {markIcon} from './icons.js';
import {profileName} from './profiles.js';

// The row's icon among other activities in a card
const ROW_ICONS = {work: 'x-office-calendar-symbolic', games: 'input-gaming-symbolic', sleep: 'weather-clear-night-symbolic'};

export class FocusMark {
    constructor(focus) {
        this._focus = focus;
        this.id = 'focus';
        this.plate = 'focus';
        this._profile = null;
        this._state = '';
        this._lines = [];
    }

    get _clock() {
        return Main.panel.statusArea.dateMenu?._clockDisplay ?? null;
    }

    // The profile on now, what the tile says of it and its lines
    show(profile, state, lines) {
        const changed = profile !== this._profile;
        this._profile = profile;
        this._state = state;
        this._lines = lines;
        if (changed && this._content) {
            // Another profile: another icon
            this._clock?.disconnectObject(this);
            retire(this._content);
            this._content = null;
        }
        if (this._content)
            this._label.text = this.text;
    }

    // The time alone, as the board has it, in the person's 12- or 24-hour clock; GNOME's clock says when the minute turns
    get text() {
        return `${profileName(this._profile)} · ${timeOfDay(new Date())}`;
    }

    get accessible() {
        return _('Focus: %s, %s').format(profileName(this._profile), this._state);
    }

    get row() {
        return {icon: ROW_ICONS[this._profile] ?? 'emblem-default-symbolic',
            text: _('Focus: %s').format(profileName(this._profile))};
    }

    pill() {
        if (!this._content) {
            const content = new St.BoxLayout({style_class: 'amethystora-pill-content amethystora-focus-mark'});
            content.add_child(markIcon(this._profile, 'amethystora-focus-mark-icon'));
            this._label = new St.Label({text: '', y_align: Clutter.ActorAlign.CENTER});
            this._label.clutter_text.use_markup = false;
            content.add_child(this._label);
            this._label.text = this.text;
            this._content = content;
            // The clock's minute, while the mark is made (GNOME's clock may tick every second: only a new minute counts)
            this._clock?.connectObject('notify::text', () => {
                const text = this.text;
                if (!this._label || this._label.text === text)
                    return;
                this._label.text = text;
                this._focus.markChanged();
            }, this);
            content.connect('destroy', () => {
                if (this._content !== content)
                    return;
                this._clock?.disconnectObject(this);
                this._content = null;
                this._label = null;
            });
        }
        return this._content;
    }

    card(api) {
        const body = new St.BoxLayout({style_class: 'amethystora-pill-card-body', orientation: Clutter.Orientation.VERTICAL});
        const title = new St.Label({style_class: 'amethystora-pill-card-label', text: profileName(this._profile)});
        body.add_child(title);
        const held = this._focus.held;
        const words = [this._state, ...this._lines];
        if (held > 0)
            words.push(ngettext('%d held while %s is on.', '%d held while %s is on.', held).format(held, profileName(this._profile)));
        // One paragraph, a line for each thing it does
        const text = new St.Label({style_class: 'amethystora-pill-card-text', text: words.join('\n')});
        text.clutter_text.use_markup = false;
        text.clutter_text.line_wrap = true;
        body.add_child(text);
        body.add_child(buttonRow([
            cardButton(_('Turn off'), () => {
                api.close();
                this._focus.pick('off');
            }, {primary: true}),
            cardButton(_('Edit in Control'), () => {
                api.close();
                Util.spawn(['amethystora-control', 'desktop', 'focus']);
            }),
        ]));
        return body;
    }

    // The mark leaves the pill
    destroy() {
        this._clock?.disconnectObject(this);
        retire(this._content);
        this._content = null;
        this._label = null;
        this._profile = null;
    }
}
