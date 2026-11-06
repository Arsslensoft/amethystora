// Put this away for tomorrow (R7-34), the pill's part of it: the question "Put "Tax return" away?" (id `putaway`, which the workspace menu's
// row asks and which shows only while the question is open), and the quiet mark that says something is put away (id `putaway-kept`, a mark
// like Focus's, last in the order), whose card lists what is put away with Bring back and Forget. What is put away, and the Vault it is
// sealed in, are workspaces/putaway.js's; this draws them and passes the person's clicks on. Nothing here runs while nothing is put away.
//
// Names are only ever drawn as text; a sealed list that has not been read yet shows as "Put away" with no name and no count.

import Clutter from 'gi://Clutter';
import Shell from 'gi://Shell';
import St from 'gi://St';

import {gettext as _, ngettext} from 'resource:///org/gnome/shell/extensions/extension.js';

import {buttonRow, cardButton, retire} from './clock.js';
import {plain} from './text.js';

const ICON = 'media-playback-pause-symbolic';
const APPS = 4;

function label(text, style) {
    const actor = new St.Label({style_class: style, text});
    actor.clutter_text.use_markup = false;
    actor.clutter_text.line_wrap = true;
    return actor;
}

// The pill's content: an icon and a line, kept and updated in place
class Mark {
    constructor() {
        this._content = new St.BoxLayout({style_class: 'amethystora-pill-content amethystora-pill-putaway'});
        this._content.add_child(new St.Icon({icon_name: ICON, icon_size: 14, y_align: Clutter.ActorAlign.CENTER}));
        this._label = new St.Label({y_align: Clutter.ActorAlign.CENTER});
        this._content.add_child(this._label);
    }

    set text(text) {
        this._label.text = text;
    }

    get actor() {
        return this._content;
    }
}

// "Put "Tax return" away?": the card the workspace menu's row opens
class Ask {
    constructor(owner) {
        this.id = 'putaway';
        this.plate = '';
        this._owner = owner;
        this._mark = new Mark();
    }

    destroy() {
        retire(this._mark.actor);
    }

    get _ask() {
        return this._owner.putAway?.pending ?? null;
    }

    get text() {
        const ask = this._ask;
        return ask ? _('Put “%s” away?').format(plain(ask.name, 24)) : '';
    }

    get accessible() {
        return this.text;
    }

    get row() {
        return {icon: ICON, text: this.text};
    }

    pill() {
        this._mark.text = this.text;
        return this._mark.actor;
    }

    card(api) {
        const ask = this._ask;
        const body = new St.BoxLayout({style_class: 'amethystora-pill-card-body amethystora-pill-news-card',
            orientation: Clutter.Orientation.VERTICAL});
        body.add_child(label(ask ? _('Put “%s” away?').format(plain(ask.name, 24)) : '', 'amethystora-pill-news-title'));
        body.add_child(label(ask
            ? ngettext('%d window on this workspace closes. One click brings it back, where it was.',
                '%d windows on this workspace close. One click brings them back, each where it was.', ask.count).format(ask.count)
            : '', 'amethystora-pill-card-text'));
        body.add_child(label(_('An app with unsaved work asks first, as closing it would. Each app opens as it starts; apps that remember their own windows bring them back.'),
            'amethystora-pill-news-once'));
        body.add_child(buttonRow([
            cardButton(_('Not now'), () => {
                api.close();
                this._owner.putAway?.cancel();
            }),
            cardButton(_('Put away'), () => {
                api.close();
                this._owner.putAway?.confirm();
            }, {primary: true}),
        ]));
        return body;
    }
}

// The mark: something is put away
class Kept {
    constructor(owner) {
        this.id = 'putaway-kept';
        this.plate = '';
        this._owner = owner;
        this._mark = new Mark();
    }

    destroy() {
        retire(this._mark.actor);
    }

    get _items() {
        return this._owner.putAway?.items ?? [];
    }

    get text() {
        const items = this._items;
        if (!items.length)
            return _('Put away');
        const name = plain(items[0].name, 24);
        return items.length === 1 ? _('%s · put away').format(name)
            : ngettext('%s and %d more · put away', '%s and %d more · put away', items.length - 1).format(name, items.length - 1);
    }

    get accessible() {
        return this._items.length ? _('Workspaces put away. Open the list to bring one back.') : _('Something is put away. Open the list.');
    }

    get row() {
        return {icon: ICON, text: this.text};
    }

    pill() {
        this._mark.text = this.text;
        return this._mark.actor;
    }

    _row(item, api) {
        const putAway = this._owner.putAway;
        const row = new St.BoxLayout({style_class: 'amethystora-pill-putaway-row'});
        const icons = new St.BoxLayout({y_align: Clutter.ActorAlign.CENTER});
        const appSystem = Shell.AppSystem.get_default();
        for (const id of [...new Set(item.windows.map(w => w.app))].slice(0, APPS)) {
            const app = appSystem.lookup_app(id);
            icons.add_child(app ? app.create_icon_texture(24) : new St.Icon({icon_name: 'application-x-executable-symbolic', icon_size: 24}));
        }
        row.add_child(icons);
        const name = label(plain(item.name, 24), 'amethystora-pill-news-title');
        name.x_expand = true;
        row.add_child(name);
        row.add_child(cardButton(_('Bring back'), () => {
            api.close();
            putAway.bringBack(item.name);
        }, {primary: true, small: true}));
        row.add_child(cardButton(_('Forget'), () => {
            api.close();
            putAway.forget(item.name);
        }, {small: true}));
        return row;
    }

    card(api) {
        const putAway = this._owner.putAway;
        const body = new St.BoxLayout({style_class: 'amethystora-pill-card-body amethystora-pill-news-card',
            orientation: Clutter.Orientation.VERTICAL});
        body.add_child(label(_('Put away'), 'amethystora-pill-news-title'));
        const items = putAway?.items ?? [];
        if (!items.length) {
            // A sealed list nobody has read: no name, no count, and the Vault is asked only by this click
            body.add_child(label(_('Something is put away. It is sealed in your Vault, so it opens when you ask.'), 'amethystora-pill-card-text'));
            const show = cardButton(_('Show it'), () => {
                api.close();
                putAway?.load().then(() => this._owner.show('putaway-kept'));
            }, {primary: true});
            body.add_child(buttonRow([cardButton(_('Open Vault'), () => putAway?.openVault()), show]));
            return body;
        }
        for (const item of items)
            body.add_child(this._row(item, api));
        const closed = putAway.stillOpen;
        if (closed > 0) {
            body.add_child(label(ngettext('%d window is still open: its app asked first.', '%d windows are still open: their apps asked first.', closed).format(closed),
                'amethystora-pill-news-once'));
        }
        const buttons = [];
        switch (putAway.problem) {
        case 'never':
            body.add_child(label(_('Kept until you log out. Set up Vault to keep it longer.'), 'amethystora-pill-news-once'));
            buttons.push(cardButton(_('Set up Vault'), () => putAway.openVault()));
            break;
        case 'closed':
            body.add_child(label(_('Kept until you log out. Open Vault to keep it longer.'), 'amethystora-pill-news-once'));
            buttons.push(cardButton(_('Open Vault'), () => putAway.openVault()));
            buttons.push(cardButton(_('Keep it'), () => {
                api.close();
                putAway.load();
            }, {primary: true}));
            break;
        case 'refused':
            body.add_child(label(_('The Vault did not keep it. It stays here until you log out.'), 'amethystora-pill-news-once'));
            buttons.push(cardButton(_('Keep it'), () => {
                api.close();
                putAway.load();
            }, {primary: true}));
            break;
        default:
            body.add_child(label(_('Sealed in your Vault. Each app opens as it starts; apps that remember their own windows bring them back.'),
                'amethystora-pill-news-once'));
        }
        if (buttons.length)
            body.add_child(buttonRow(buttons));
        return body;
    }
}

// The two cards as one pill part
export class PutAwayCards {
    // `workspaces` gives the workspaces module's PutAway (or null), `openCard` opens an activity's card
    constructor(activities, workspaces, openCard) {
        this._activities = activities;
        this._workspaces = workspaces;
        this._openCard = openCard;
        this._ask = new Ask(this);
        this._kept = new Kept(this);
    }

    get putAway() {
        return this._workspaces() ?? null;
    }

    enable() {
        this.refresh();
    }

    disable() {
        this._activities.set('putaway', null);
        this._activities.set('putaway-kept', null);
        this._ask.destroy();
        this._kept.destroy();
    }

    // What is put away, or being asked, changed
    refresh() {
        const putAway = this.putAway;
        this._apply('putaway', this._ask, !!putAway?.pending);
        this._apply('putaway-kept', this._kept, !!putAway?.anything);
    }

    _apply(id, activity, on) {
        if (on && this._activities.get(id) !== activity)
            this._activities.set(id, activity);
        else if (!on && this._activities.get(id) === activity)
            this._activities.set(id, null);
        else if (on)
            this._activities.update(id);
    }

    show(id) {
        this._openCard(id);
    }
}
