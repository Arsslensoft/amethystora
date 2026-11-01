// The two questions of an app's menu in the grid: a name of the account's own for the app, and its icon from another
// pack or a picture. Both are GNOME Shell's own modal dialogs, light or dark with the shell.

import Clutter from 'gi://Clutter';
import Gio from 'gi://Gio';
import GObject from 'gi://GObject';
import St from 'gi://St';

import {gettext as _} from 'resource:///org/gnome/shell/extensions/extension.js';
import * as Dialog from 'resource:///org/gnome/shell/ui/dialog.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import * as ModalDialog from 'resource:///org/gnome/shell/ui/modalDialog.js';

import {giveBackTextInput} from '../emoji/paste.js';

const NAME_MAX = 64;
const TILE = 64;

function styleClass() {
    return Main.getStyleVariant() === 'light'
        ? 'amethystora-appgrid-dialog amethystora-appgrid-light'
        : 'amethystora-appgrid-dialog';
}

// The name: null when the person cancels, '' for the app's own name again, else what was typed. The text field takes
// the keyboard, and gives the window its input method back when the dialog closes.
export function askName(own, current) {
    const dialog = new ModalDialog.ModalDialog({styleClass: styleClass()});
    const answer = new Promise(resolve => {
        dialog.contentLayout.add_child(new Dialog.MessageDialogContent({
            title: _('Rename %s').format(own),
            description: _('Only the name under its icon changes. Its launcher stays as it is.'),
        }));
        const entry = new St.Entry({
            style_class: 'amethystora-appgrid-entry',
            text: current,
            hint_text: own,
            can_focus: true,
            x_expand: true,
        });
        entry.clutter_text.max_length = NAME_MAX;
        entry.clutter_text.use_markup = false;
        dialog.contentLayout.add_child(entry);

        let chosen = null;
        const done = value => {
            chosen = value;
            dialog.close();
        };
        entry.clutter_text.connect('activate', () => done(entry.get_text()));
        dialog.addButton({label: _('Use its own name'), action: () => done('')});
        dialog.addButton({label: _('Cancel'), action: () => dialog.close(), key: Clutter.KEY_Escape});
        dialog.addButton({label: _('Rename'), action: () => done(entry.get_text()), default: true});
        dialog.setInitialKeyFocus(entry.clutter_text);
        dialog.connect('closed', () => {
            giveBackTextInput();
            resolve(chosen);
        });
        dialog.open();
        entry.clutter_text.set_selection(0, -1);
    });
    return {dialog, answer};
}

// One choice of icon: the whole tile is the button
const Tile = GObject.registerClass(
class Tile extends St.Button {
    _init(picture, title) {
        super._init({
            style_class: 'amethystora-appgrid-icon-choice',
            can_focus: true,
            accessible_name: title,
        });
        const box = new St.BoxLayout({orientation: Clutter.Orientation.VERTICAL, style_class: 'amethystora-appgrid-icon-box'});
        const bin = new St.Bin({child: picture, x_align: Clutter.ActorAlign.CENTER});
        box.add_child(bin);
        const label = new St.Label({text: title, style_class: 'amethystora-appgrid-icon-title', x_align: Clutter.ActorAlign.CENTER});
        label.clutter_text.use_markup = false;
        box.add_child(label);
        this.set_child(box);
    }
});

// The icon: null when the person cancels, 'own' for the app's own, 'picture' for a picture to pick, or a pack's name
export function askIcon(name, ownPicture, packs, canPick) {
    const dialog = new ModalDialog.ModalDialog({styleClass: styleClass()});
    const answer = new Promise(resolve => {
        dialog.contentLayout.add_child(new Dialog.MessageDialogContent({
            title: _('Icon for %s').format(name),
            description: packs.length
                ? _('The same app drawn by another of the icon packs, or a picture of your own.')
                : _('No icon pack draws this app. A picture of your own can stand in for its icon.'),
        }));
        let chosen = null;
        const done = value => {
            chosen = value;
            dialog.close();
        };
        const row = new St.BoxLayout({style_class: 'amethystora-appgrid-icon-choices', x_align: Clutter.ActorAlign.CENTER});
        const tiles = [];
        if (ownPicture) {
            ownPicture.icon_size = TILE;
            tiles.push([new Tile(ownPicture, _('Its own')), 'own']);
        }
        for (const {pack, file, title} of packs) {
            const picture = new St.Icon({gicon: Gio.FileIcon.new(Gio.File.new_for_path(file)), icon_size: TILE});
            tiles.push([new Tile(picture, title), pack]);
        }
        for (const [tile, value] of tiles) {
            tile.connect('clicked', () => done(value));
            row.add_child(tile);
        }
        dialog.contentLayout.add_child(row);
        if (canPick)
            dialog.addButton({label: _('A picture of your own…'), action: () => done('picture')});
        dialog.addButton({label: _('Reset'), action: () => done('own')});
        dialog.addButton({label: _('Cancel'), action: () => dialog.close(), key: Clutter.KEY_Escape, default: true});
        if (tiles.length)
            dialog.setInitialKeyFocus(tiles[0][0]);
        dialog.connect('closed', () => resolve(chosen));
        dialog.open();
    });
    return {dialog, answer};
}
