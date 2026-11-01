// The two questions an uninstall can ask: whether to remove the app and keep or delete its data, and, for an app
// installed twice, a note that Bazaar is where to choose which copy goes. Both are GNOME Shell's own modal dialogs.

import Atk from 'gi://Atk';
import Clutter from 'gi://Clutter';
import GObject from 'gi://GObject';
import St from 'gi://St';

import {gettext as _} from 'resource:///org/gnome/shell/extensions/extension.js';
import * as Dialog from 'resource:///org/gnome/shell/ui/dialog.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import * as ModalDialog from 'resource:///org/gnome/shell/ui/modalDialog.js';

// The list's colours follow the shell's light or dark style, as the dialog round it does
function styleClass() {
    return Main.getStyleVariant() === 'light'
        ? 'amethystora-bazaar-dialog amethystora-bazaar-light'
        : 'amethystora-bazaar-dialog';
}

// One choice of the two: the whole row is the button
const Choice = GObject.registerClass(
class Choice extends St.Button {
    _init(title, detail) {
        super._init({
            style_class: 'amethystora-bazaar-choice',
            can_focus: true,
            x_expand: true,
            accessible_role: Atk.Role.RADIO_BUTTON,
            accessible_name: title,
        });
        const box = new St.BoxLayout({style_class: 'amethystora-bazaar-choice-box', x_expand: true});
        this._mark = new St.Widget({style_class: 'amethystora-bazaar-mark', y_align: Clutter.ActorAlign.CENTER});
        box.add_child(this._mark);
        const text = new St.BoxLayout({orientation: Clutter.Orientation.VERTICAL, x_expand: true});
        text.add_child(new St.Label({text: title, style_class: 'amethystora-bazaar-choice-title'}));
        const label = new St.Label({text: detail, style_class: 'amethystora-bazaar-choice-detail'});
        label.clutter_text.line_wrap = true;
        text.add_child(label);
        box.add_child(text);
        this.set_child(box);
    }
});

function addTitle(dialog, name, description) {
    const content = new Dialog.MessageDialogContent({
        title: _('Uninstall %s?').format(name),
        description,
    });
    dialog.contentLayout.add_child(content);
}

// The dialog, and its answer: null when the person cancels, else whether the app's data goes with it. It says which
// Flatpak is removed as well as what the launcher calls itself: a launcher's name is whatever its maker wrote.
export function askToUninstall(name, id) {
    const dialog = new ModalDialog.ModalDialog({styleClass: styleClass()});
    const answer = new Promise(resolve => {
        addTitle(dialog, name, [
            _('Once it is uninstalled, the app can no longer be used.'),
            _('Flatpak app ID: %s').format(id),
        ].join('\n'));

        const list = new St.BoxLayout({
            style_class: 'amethystora-bazaar-choices',
            orientation: Clutter.Orientation.VERTICAL,
            x_align: Clutter.ActorAlign.CENTER,
        });
        const keep = new Choice(_('Keep its data'),
            _('Its settings and what it saved come back if you install it again.'));
        const erase = new Choice(_('Delete its data'),
            _('Its settings and everything it saved are removed for good, which frees space.'));
        // A group of radio buttons: Tab reaches the chosen one only, and the arrow keys move the choice
        let deleteData = false;
        const choose = (choice, focus) => {
            deleteData = choice === erase;
            keep.checked = !deleteData;
            erase.checked = deleteData;
            keep.can_focus = keep.checked;
            erase.can_focus = erase.checked;
            if (focus)
                choice.grab_key_focus();
        };
        const arrows = [Clutter.KEY_Up, Clutter.KEY_Down, Clutter.KEY_Left, Clutter.KEY_Right];
        for (const choice of [keep, erase]) {
            choice.connect('clicked', () => choose(choice, true));
            choice.connect('key-press-event', (actor, event) => {
                if (!arrows.includes(event.get_key_symbol()))
                    return Clutter.EVENT_PROPAGATE;
                choose(choice === keep ? erase : keep, true);
                return Clutter.EVENT_STOP;
            });
        }
        list.add_child(keep);
        list.add_child(new St.Widget({style_class: 'amethystora-bazaar-line'}));
        list.add_child(erase);
        dialog.contentLayout.add_child(list);
        choose(keep, false);

        let deleteChosen = null;
        dialog.addButton({
            label: _('Cancel'),
            action: () => dialog.close(),
            key: Clutter.KEY_Escape,
        });
        const uninstall = dialog.addButton({
            label: _('Uninstall'),
            action: () => {
                deleteChosen = deleteData;
                dialog.close();
            },
        });
        uninstall.add_style_class_name('amethystora-bazaar-uninstall');
        dialog.setInitialKeyFocus(keep);
        dialog.connect('closed', () => resolve(deleteChosen));
        dialog.open();
    });
    return {dialog, answer};
}

// For an app installed both for the account and for the whole machine: nothing is removed here
export function explainTwoCopies(name) {
    const dialog = new ModalDialog.ModalDialog({styleClass: styleClass()});
    addTitle(dialog, name, _('%s is installed both for you and for everyone on this computer. Use Bazaar to choose which one to remove.').format(name));
    dialog.addButton({
        label: _('OK'),
        action: () => dialog.close(),
        key: Clutter.KEY_Escape,
        default: true,
    });
    dialog.open();
    return dialog;
}
