// The phone in the top bar: with one phone, a capsule of its battery and its signal (board 32, second card), or its icon
// when it has said neither; with several, the icon alone. A menu with each connected phone's name, charge, signal and
// what can be done with it. The rows show only while the phone can take the
// request, and do nothing themselves: they hand the choice to the module.

import Clutter from 'gi://Clutter';
import Gio from 'gi://Gio';
import GObject from 'gi://GObject';
import St from 'gi://St';

import {gettext as _, ngettext} from 'resource:///org/gnome/shell/extensions/extension.js';
import * as PanelMenu from 'resource:///org/gnome/shell/ui/panelMenu.js';
import * as PopupMenu from 'resource:///org/gnome/shell/ui/popupMenu.js';

import {BatteryGlyph, SignalBars} from './glyphs.js';

// The icons of the Adwaita theme, in the order they are tried: its newer name for a phone, then its older
const BAR_ICON = ['smartphone-symbolic', 'phone-symbolic'];
const TYPE_ICONS = {
    phone: BAR_ICON,
    tablet: ['tablet-symbolic'],
    laptop: ['laptop-symbolic', 'computer-symbolic'],
    desktop: ['computer-symbolic'],
    tv: ['tv-symbolic'],
};

// The battery in the menu is quieter than the name beside it, out of 255
const QUIET = 170;

let percentFormat = null;

// 87 as the session's language writes "87%"
function percent(level) {
    percentFormat ??= new Intl.NumberFormat(undefined, {style: 'percent', maximumFractionDigits: 0});
    return percentFormat.format(level / 100);
}

// What a screen reader says of the capsule, leaving out what the phone has not said. The battery and the signal are
// each a phrase of their own, so that each takes the plural form of its own number
export function capsuleName(level, signal) {
    // ngettext takes no number below 0, which is what the phone has not said
    const battery = level >= 0
        // Translators: the phone's charge, said by a screen reader: "battery 54 per cent"
        ? ngettext('battery %d per cent', 'battery %d per cent', level).format(level) : '';
    const bars = signal >= 0
        // Translators: the phone's mobile signal, said by a screen reader: "signal 3 of 4 bars"
        ? ngettext('signal %d of 4 bars', 'signal %d of 4 bars', signal).format(signal) : '';
    if (level >= 0 && signal >= 0) {
        // Translators: the phone in the top bar, said by a screen reader: its battery, then its signal
        return _('Phone: %s, %s').format(battery, bars);
    }
    // Translators: the phone in the top bar, said by a screen reader: its battery or its signal
    return _('Phone: %s').format(level >= 0 ? battery : bars);
}

// One phone's part of the menu: its title, then its rows, then a line under them. The line is the menu's own item and
// not the section's, since the menu hides a separator that ends a section.
class Section {
    constructor(device, actions, bolt) {
        this.menu = new PopupMenu.PopupMenuSection();
        this.separator = new PopupMenu.PopupSeparatorMenuItem();

        const title = new PopupMenu.PopupBaseMenuItem({reactive: false, can_focus: false});
        title.add_style_class_name('amethystora-connect-title');
        this._type = new St.Icon({style_class: 'popup-menu-icon'});
        this._name = new St.Label({
            style_class: 'amethystora-connect-name',
            x_expand: true,
            y_align: Clutter.ActorAlign.CENTER,
        });
        this._level = new St.Label({
            style_class: 'amethystora-connect-battery',
            y_align: Clutter.ActorAlign.CENTER,
            opacity: QUIET,
        });
        this._charge = new St.Icon({gicon: bolt, style_class: 'popup-menu-icon amethystora-connect-bolt'});
        // The signal after the battery, its network in the battery's quiet style
        this._bars = new SignalBars();
        this._network = new St.Label({
            style_class: 'amethystora-connect-network',
            y_align: Clutter.ActorAlign.CENTER,
            opacity: QUIET,
        });
        for (const actor of [this._type, this._name, this._level, this._charge, this._bars, this._network])
            title.add_child(actor);
        this.menu.addMenuItem(title);

        // Each row is shown only while the phone says it takes the request
        this._rows = [
            [_('Send files…'), 'share', true, () => actions.sendFiles(device)],
            [_('Send a link'), 'share', false, () => actions.sendLink(device)],
            [_('Ping'), 'ping', false, () => actions.ping(device)],
            // Even when it is silent, until Found it is pressed on it (plans/ui-ideas/spec-phone.md 2)
            [_('Ring the phone'), 'ring', false, () => actions.ring(device)],
            [_('Messages'), 'sms', true, () => actions.messages(device)],
        ].map(([label, feature, app, run]) => {
            const item = new PopupMenu.PopupMenuItem(label);
            item.connect('activate', run);
            this.menu.addMenuItem(item);
            return {item, feature, app};
        });
    }

    update(device) {
        this._type.gicon = Gio.ThemedIcon.new_from_names(TYPE_ICONS[device.type]);
        this._name.text = device.name;
        const level = device.level;
        this._level.visible = level >= 0;
        this._level.text = level >= 0 ? percent(level) : '';
        this._charge.visible = device.charging;
        const signal = device.signal;
        this._bars.visible = signal >= 0;
        this._bars.level = Math.max(signal, 0);
        this._network.visible = signal >= 0 && device.network !== '';
        this._network.text = signal >= 0 ? device.network : '';

        const features = device.features;
        for (const {item, feature, app} of this._rows)
            item.visible = features.has(feature) && (!app || device.launchable);
    }

    destroy() {
        this.menu.destroy();
        this.separator.destroy();
    }
}

export const Indicator = GObject.registerClass(
class PhoneIndicator extends PanelMenu.Button {
    /**
     * @param {object} actions - what a row does, each given the phone: sendFiles, sendLink, ping, ring, messages, and
     *     phones() for the app's window
     * @param {Gio.Icon} bolt - the mark of a phone that is charging
     */
    _init(actions, bolt) {
        super._init(0.5, _('Phones'), false);
        this.add_style_class_name('amethystora-connect-button');
        this.menu.actor.add_style_class_name('amethystora-connect-menu');
        this._actions = actions;
        this._bolt = bolt;
        this._sections = new Map();

        const box = new St.BoxLayout({style_class: 'amethystora-connect-box', y_align: Clutter.ActorAlign.CENTER});
        this._icon = new St.Icon({
            gicon: Gio.ThemedIcon.new_from_names(BAR_ICON),
            style_class: 'system-status-icon',
        });
        this._kind = 'phone';
        box.add_child(this._icon);
        // One phone that has said its battery or its signal: the capsule, battery outline, percentage, charging mark
        // and bars, each shown only while known
        this._capsule = new St.BoxLayout({
            style_class: 'amethystora-connect-capsule',
            y_align: Clutter.ActorAlign.CENTER,
            visible: false,
        });
        this._battery = new BatteryGlyph();
        this._level = new St.Label({
            style_class: 'amethystora-connect-level',
            y_align: Clutter.ActorAlign.CENTER,
        });
        this._charge = new St.Icon({
            gicon: bolt,
            style_class: 'system-status-icon amethystora-connect-bolt',
        });
        this._bars = new SignalBars();
        for (const actor of [this._battery, this._level, this._charge, this._bars])
            this._capsule.add_child(actor);
        box.add_child(this._capsule);
        this.add_child(box);

        const phones = new PopupMenu.PopupMenuItem(_('Phones…'));
        phones.connect('activate', () => actions.phones());
        this.menu.addMenuItem(phones);
    }

    /**
     * The phones now connected, in the order they are listed. A phone's part of the menu is made when it first
     * appears and taken away when it leaves; the rest is changed where it stands.
     *
     * @param {import('./link.js').Device[]} devices
     */
    update(devices) {
        const present = new Set(devices.map(device => device.path));
        for (const [path, section] of this._sections) {
            if (!present.has(path)) {
                section.destroy();
                this._sections.delete(path);
            }
        }
        devices.forEach((device, position) => {
            let section = this._sections.get(device.path);
            if (!section) {
                section = new Section(device, this._actions, this._bolt);
                this._sections.set(device.path, section);
                this.menu.addMenuItem(section.menu, 2 * position);
                this.menu.addMenuItem(section.separator, 2 * position + 1);
            } else if (this.menu.box.get_children().indexOf(section.menu.actor) !== 2 * position) {
                // A phone with a new name stands elsewhere in the order
                this.menu.moveMenuItem(section.menu, 2 * position);
                this.menu.moveMenuItem(section.separator, 2 * position + 1);
            }
            section.update(device);
        });

        // One phone is its battery and signal, in the bar; several are in the menu
        const only = devices.length === 1 ? devices[0] : null;
        const level = only?.level ?? -1;
        const signal = only?.signal ?? -1;
        const capsule = level >= 0 || signal >= 0;
        this._capsule.visible = capsule;
        this._icon.visible = !capsule;
        // Another computer (spec-r4-later.md A.5) is drawn as one: the only device's own kind, or a computer when every
        // device connected is one
        const computer = devices.length > 0 && devices.every(device => ['desktop', 'laptop'].includes(device.type));
        const kind = TYPE_ICONS[only?.type] ? only.type : computer ? 'desktop' : 'phone';
        if (this._kind !== kind) {
            this._kind = kind;
            this._icon.gicon = Gio.ThemedIcon.new_from_names(TYPE_ICONS[kind]);
        }
        this._battery.visible = level >= 0;
        this._level.visible = level >= 0;
        this._level.text = level >= 0 ? percent(level) : '';
        this._charge.visible = level >= 0 && only.charging;
        this._bars.visible = signal >= 0;
        this._bars.level = Math.max(signal, 0);
        this.accessible_name = capsule ? capsuleName(level, signal) : only?.name ?? _('Phones');
    }
});
