// Charged by the time you leave (R7-70), the pill's part of it: while the battery is held at its limit for the night the pill says so and
// when it will be full, with Full now, just this once; and, once after a restart, that the pattern is sealed and the computer is charging fully
// (power/charge.js decides, this draws). The times are shown from memory, never written; nothing is shown when the part is off or the
// computer has no limit to hold.

import Clutter from 'gi://Clutter';
import St from 'gi://St';

import {gettext as _} from 'resource:///org/gnome/shell/extensions/extension.js';

import {buttonRow, cardButton, retire} from './clock.js';
import {timeOfDay} from './text.js';

const ICON = 'battery-full-charged-symbolic';

function label(text, style) {
    const actor = new St.Label({style_class: style, text});
    actor.clutter_text.use_markup = false;
    actor.clutter_text.line_wrap = true;
    return actor;
}

class Mark {
    constructor(owner) {
        this.id = 'chargeby';
        this.plate = '';
        this._owner = owner;
        this._content = new St.BoxLayout({style_class: 'amethystora-pill-content amethystora-pill-chargeby'});
        this._content.add_child(new St.Icon({icon_name: ICON, icon_size: 14, y_align: Clutter.ActorAlign.CENTER}));
        this._label = new St.Label({y_align: Clutter.ActorAlign.CENTER});
        this._content.add_child(this._label);
    }

    destroy() {
        retire(this._content);
    }

    get _view() {
        return this._owner.charge?.view ?? null;
    }

    get text() {
        const view = this._view;
        if (!view)
            return '';
        if (view.stuck)
            return _('Charge limit stays on');
        if (view.holding)
            return view.limit > 0 ? _('Held at %d%%').format(view.limit) : _('Held for the night');
        if (view.refused)
            return _('No charge limit');
        return _('Charging fully');
    }

    get accessible() {
        return this.text;
    }

    get row() {
        return {icon: ICON, text: this.text};
    }

    pill() {
        this._label.text = this.text;
        return this._content;
    }

    card(api) {
        const view = this._view;
        const charge = this._owner.charge;
        const body = new St.BoxLayout({style_class: 'amethystora-pill-card-body amethystora-pill-news-card',
            orientation: Clutter.Orientation.VERTICAL});
        body.add_child(label(this.text, 'amethystora-pill-news-title'));
        if (view?.holding && !view.stuck) {
            body.add_child(label(_('Your battery lasts longer when it does not sit full.'), 'amethystora-pill-card-text'));
            if (view.leaveAt) {
                body.add_child(label(_('Full by %s. You usually leave about then.').format(timeOfDay(new Date(view.leaveAt))),
                    'amethystora-pill-news-once'));
            }
            body.add_child(buttonRow([cardButton(_('Full now, just this once'), () => {
                api.close();
                charge.fullNow();
            }, {primary: true})]));
        } else if (view?.leaveAt) {
            body.add_child(label(_('You usually leave about %s, which is soon, so it is charging fully.').format(timeOfDay(new Date(view.leaveAt))),
                'amethystora-pill-card-text'));
        } else if (view?.stuck) {
            body.add_child(label(_('The battery was held at its limit and the system did not let this take the limit off. It tries again whenever anything changes, and when you start next.'),
                'amethystora-pill-card-text'));
        } else if (view?.refused) {
            body.add_child(label(_('This laptop did not take the change, so nothing is held. It charges fully.'), 'amethystora-pill-card-text'));
        } else if (view?.sealed) {
            body.add_child(label(_('What you usually do is sealed in your Vault, so it opens when you ask. Open it and the battery can wait at its limit overnight.'),
                'amethystora-pill-card-text'));
            body.add_child(buttonRow([
                cardButton(_('Open Vault'), () => charge.openVault()),
                cardButton(_('Use my pattern'), () => {
                    api.close();
                    charge.load();
                }, {primary: true}),
            ]));
        } else {
            body.add_child(label(_('It is still learning when you leave: it needs a few nights on mains, then it can hold the charge and finish it just before you go. Until then it charges fully.'),
                'amethystora-pill-card-text'));
            if (view?.problem !== null) {
                body.add_child(label(_('What it learned is kept until you log out. Open Vault to keep it longer.'), 'amethystora-pill-news-once'));
                body.add_child(buttonRow([cardButton(_('Open Vault'), () => charge.openVault()),
                    cardButton(_('Keep it'), () => {
                        api.close();
                        charge.keepPattern();
                    }, {primary: true})]));
            }
        }
        return body;
    }
}

export class ChargeCards {
    // `power` gives the power module's ChargeBy (or null), `openCard` opens an activity's card
    constructor(activities, power, openCard) {
        this._activities = activities;
        this._power = power;
        this._openCard = openCard;
        this._mark = new Mark(this);
    }

    get charge() {
        return this._power() ?? null;
    }

    enable() {
        this.refresh();
    }

    disable() {
        this._activities.set('chargeby', null);
        this._mark.destroy();
    }

    // Shown while it holds the battery, or says what it does not know, or that the laptop refused
    refresh() {
        const view = this.charge?.view;
        const on = !!view?.supported && view.onMains && (view.holding || view.refused || view.stuck || view.said);
        if (on && this._activities.get('chargeby') !== this._mark)
            this._activities.set('chargeby', this._mark);
        else if (!on && this._activities.get('chargeby') === this._mark)
            this._activities.set('chargeby', null);
        else if (on)
            this._activities.update('chargeby');
    }

    show() {
        this._openCard('chargeby');
    }
}
