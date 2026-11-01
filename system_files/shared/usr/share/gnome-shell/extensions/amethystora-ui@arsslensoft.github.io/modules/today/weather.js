// The weather, offered and off. Off (as the image ships), GNOME's weather section in the calendar menu is hidden and
// its weather client fetches nothing, whatever opens the menu or changes the place in Weather; in its place a card
// says the weather is off and what turning it on sends, and asks before it turns it on. On, GNOME's section is GNOME's,
// and the temperature and the sky show beside the clock in the top bar, refreshed by GNOME's own client when the menu
// opens, when the computer wakes, and once an hour while it is on.
//
// GNOME Shell's own client may still fetch once as the session starts, before any extension runs, if a place was set
// in Weather: that is GNOME's, and "What this machine sends" says so.

import Clutter from 'gi://Clutter';
import GLib from 'gi://GLib';
import GWeather from 'gi://GWeather';
import Pango from 'gi://Pango';
import St from 'gi://St';

import {gettext as _} from 'resource:///org/gnome/shell/extensions/extension.js';
import * as LoginManager from 'resource:///org/gnome/shell/misc/loginManager.js';

// The refresh while the weather is on: what the feature is for, not a poll of anything on this machine
const REFRESH_S = 3600;

// An instance method wrapped: `around(original, args)` runs in its place; the function returned puts it back
function wrap(object, name, around) {
    const original = object[name];
    if (typeof original !== 'function')
        return null;
    const own = Object.prototype.hasOwnProperty.call(object, name);
    object[name] = function (...args) {
        return around.call(this, original, args);
    };
    return () => {
        if (own)
            object[name] = original;
        else
            delete object[name];
    };
}

export class Weather {
    constructor(dateMenu, settings) {
        this._dateMenu = dateMenu;
        this._settings = settings;
    }

    get _on() {
        return this._settings?.get_boolean('weather') ?? false;
    }

    enable() {
        this._unwraps = [];
        this._section = this._dateMenu._weatherItem ?? null;
        this._client = this._section?._weatherClient ?? null;
        if (!this._section || !this._client)
            throw new Error('the calendar menu has no weather section');
        const self = this;
        // Off, nothing is fetched: neither GNOME's refresh nor a place changed in Weather reaches the network
        this._unwraps.push(wrap(this._client, 'update', function (original, args) {
            return self._on ? original.apply(this, args) : undefined;
        }));
        this._unwraps.push(wrap(this._client, '_loadInfo', function (original, args) {
            return self._on ? original.apply(this, args) : undefined;
        }));
        // GNOME's section shows only while on and with a place to show. GNOME connected its section to the client's
        // `changed` with the method bound at its construction, which this wrap does not reach: GNOME's handler still
        // shows the section on every change, and this module's own handler, connected after it below, runs the wrapped
        // method straight after and hides it again, before anything is drawn
        this._unwraps.push(wrap(this._section, '_sync', function (original, args) {
            const result = original.apply(this, args);
            if (!self._on || !self._client.hasLocation)
                this.visible = false;
            return result;
        }));

        this._buildCard();
        this._buildPill();
        this._asking = false;

        this._client.connectObject('changed', () => this._sync(), this);
        this._settings.connectObject('changed::weather', () => this._keyChanged(), this);
        this._dateMenu.menu.connectObject('open-state-changed', (menu, open) => {
            // A question left open goes with the menu
            if (!open && this._asking) {
                this._asking = false;
                this._sync();
            }
        }, this);
        this._keyChanged(false);
    }

    disable() {
        this._client?.disconnectObject(this);
        this._settings?.disconnectObject(this);
        this._dateMenu.menu.disconnectObject(this);
        this._setRefresh(false);
        for (const unwrap of this._unwraps ?? [])
            unwrap?.();
        this._unwraps = null;
        this._card?.destroy();
        this._card = null;
        this._pill?.destroy();
        this._pill = null;
        // GNOME's section as GNOME has it
        this._section?._sync?.();
        this._section = null;
        this._client = null;
    }

    cardShown() {
        return this._card?.visible ?? false;
    }

    pillShown() {
        return this._pill?.visible ?? false;
    }

    // ---- The card in the menu ----

    _buildCard() {
        // A row with the icon, the words and the offer's button; under it the question's two buttons, or the place's
        this._card = new St.BoxLayout({
            style_class: 'amethystora-today-weather-off',
            orientation: Clutter.Orientation.VERTICAL,
            x_expand: true,
            visible: false,
        });
        const row = new St.BoxLayout({style_class: 'amethystora-today-weather-row', x_expand: true});
        this._card.add_child(row);
        this._cardIcon = new St.Icon({
            icon_name: 'weather-few-clouds-symbolic',
            style_class: 'amethystora-today-weather-icon',
            y_align: Clutter.ActorAlign.CENTER,
        });
        row.add_child(this._cardIcon);
        const words = new St.BoxLayout({
            orientation: Clutter.Orientation.VERTICAL,
            x_expand: true,
            y_align: Clutter.ActorAlign.CENTER,
        });
        this._first = new St.Label({style_class: 'amethystora-today-weather-first'});
        this._rest = new St.Label({style_class: 'amethystora-today-weather-rest'});
        for (const label of [this._first, this._rest]) {
            label.clutter_text.line_wrap = true;
            label.clutter_text.ellipsize = Pango.EllipsizeMode.NONE;
            words.add_child(label);
        }
        row.add_child(words);
        this._inline = new St.BoxLayout({style_class: 'amethystora-today-weather-buttons', y_align: Clutter.ActorAlign.CENTER});
        row.add_child(this._inline);
        this._buttons = new St.BoxLayout({style_class: 'amethystora-today-weather-buttons', x_align: Clutter.ActorAlign.END});
        this._card.add_child(this._buttons);
        // In the menu where GNOME's weather is
        this._section.get_parent().insert_child_above(this._card, this._section);
    }

    _button(label, accent, onClick, box = this._buttons) {
        const button = new St.Button({
            label,
            can_focus: true,
            style_class: accent ? 'amethystora-today-weather-button amethystora-today-weather-accent'
                : 'amethystora-today-weather-button',
        });
        button.connect('clicked', onClick);
        box.add_child(button);
        return button;
    }

    // What the card says: that it is off, the question, or where to choose a place
    _fillCard(state) {
        if (this._cardState === state)
            return;
        this._cardState = state;
        this._buttons.destroy_all_children();
        this._inline.destroy_all_children();
        this._buttons.visible = state !== 'offer';
        // The question is read as a sentence, not a heading
        if (state === 'ask')
            this._first.add_style_class_name('amethystora-today-weather-plain');
        else
            this._first.remove_style_class_name('amethystora-today-weather-plain');
        switch (state) {
        case 'offer':
            this._first.text = _('Weather is off until chosen.');
            this._rest.text = _('It sends your place to a weather service.');
            this._rest.show();
            this._button(_('Turn on…'), false, () => {
                this._asking = true;
                this._sync();
            }, this._inline);
            break;
        case 'ask':
            this._first.text = _('The place you chose in Weather goes to the weather services GNOME uses, each time the weather is shown.');
            this._rest.hide();
            this._button(_('Not now'), false, () => {
                this._asking = false;
                this._sync();
            });
            this._button(_('Turn on'), true, () => {
                this._asking = false;
                // The person has just read what it sends: the shell writes its own key
                this._settings.set_boolean('weather', true);
            });
            break;
        case 'place':
            this._first.text = _('Choose a place in Weather');
            this._rest.hide();
            this._button(_('Open Weather'), true, () => {
                this._dateMenu.menu.close();
                this._client.activateApp();
            });
            break;
        }
    }

    // ---- The weather beside the clock ----

    _buildPill() {
        this._pill = new St.BoxLayout({style_class: 'amethystora-today-weather-pill', visible: false,
            y_align: Clutter.ActorAlign.CENTER});
        this._pillIcon = new St.Icon({style_class: 'amethystora-today-weather-pill-icon', y_align: Clutter.ActorAlign.CENTER});
        this._pillText = new St.Label({style_class: 'amethystora-today-weather-pill-text', y_align: Clutter.ActorAlign.CENTER});
        this._pill.add_child(this._pillIcon);
        this._pill.add_child(this._pillText);
        // After the clock's text, in the box GNOME gives the clock (the live pill keeps that box as it is)
        const clock = this._dateMenu._clockDisplay;
        const box = clock?.get_parent();
        if (box)
            box.insert_child_above(this._pill, clock);
    }

    // ---- Following the key and GNOME's client ----

    _keyChanged(fetch = true) {
        const on = this._on;
        this._setRefresh(on);
        if (on && fetch)
            this._client.update();
        this._sync();
    }

    _setRefresh(on) {
        if (on && !this._timer) {
            this._timer = GLib.timeout_add_seconds(GLib.PRIORITY_DEFAULT, REFRESH_S, () => {
                this._client?.update();
                return GLib.SOURCE_CONTINUE;
            });
            // Awake again: what the weather is now
            LoginManager.getLoginManager().connectObject('prepare-for-sleep', (manager, sleeping) => {
                if (!sleeping)
                    this._client?.update();
            }, this);
        } else if (!on && this._timer) {
            GLib.source_remove(this._timer);
            this._timer = 0;
            LoginManager.getLoginManager().disconnectObject(this);
        }
    }

    _sync() {
        if (!this._card)
            return;
        const on = this._on;
        const available = this._client.available;
        const located = this._client.hasLocation;
        // GNOME's section follows its own rule, wrapped above
        this._section._sync();
        let card = null;
        if (available && !on)
            card = this._asking ? 'ask' : 'offer';
        else if (available && on && !located)
            card = 'place';
        if (card)
            this._fillCard(card);
        this._card.visible = card !== null;
        this._syncPill(on && located);
    }

    _syncPill(wanted) {
        const info = this._client.info;
        let temp = null;
        if (wanted && !this._client.loading && info?.is_valid()) {
            const [ok, value] = info.get_value_temp(GWeather.TemperatureUnit.DEFAULT);
            if (ok)
                temp = value;
        }
        this._pill.visible = temp !== null;
        if (temp === null)
            return;
        this._pillIcon.icon_name = info.get_symbolic_icon_name();
        // A whole number and the degree sign, in the unit of the locale
        this._pillText.text = _('%d°').format(Math.round(temp));
        this._pill.accessible_name = this._pillText.text;
    }
}
