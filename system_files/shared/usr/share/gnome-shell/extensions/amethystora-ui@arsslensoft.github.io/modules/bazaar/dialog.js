// The two questions an uninstall can ask: whether to remove the app and keep or delete its data, and, for an app
// installed twice, a note that Bazaar is where to choose which copy goes. Both are GNOME Shell's own modal dialogs.

import Atk from 'gi://Atk';
import Clutter from 'gi://Clutter';
import GLib from 'gi://GLib';
import GObject from 'gi://GObject';
import St from 'gi://St';

import {gettext as _, ngettext} from 'resource:///org/gnome/shell/extensions/extension.js';
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
        this.detail = label;
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

// Uninstall means gone (R7-41): what the app leaves behind, with a switch for it, on as it opens. The dialog shows at once, and
// fill() gives it the size and the dates when the helper has read them, leaving a tick the person has already changed as it is.
// Its answer: null when the person keeps the app, else whether the data goes with it, and whether it had none.
export function askToRemove(name, id) {
    const dialog = new ModalDialog.ModalDialog({styleClass: styleClass()});
    const content = new Dialog.MessageDialogContent({title: _('Remove %s?').format(name), description: _('Flatpak app ID: %s').format(id)});
    dialog.contentLayout.add_child(content);

    const list = new St.BoxLayout({
        style_class: 'amethystora-bazaar-choices',
        orientation: Clutter.Orientation.VERTICAL,
        x_align: Clutter.ActorAlign.CENTER,
    });
    const app = new Choice(_('The app'), _('Looking…'));
    // Marked as going, like the data's row; keeping it is the Keep button
    app.checked = true;
    app.reactive = false;
    app.can_focus = false;
    app.accessible_role = Atk.Role.LABEL;
    const data = new Choice(_('Its settings and data'), _('Looking…'));
    data.accessible_role = Atk.Role.CHECK_BOX;
    data.toggle_mode = true;
    data.checked = true;
    list.add_child(app);
    list.add_child(new St.Widget({style_class: 'amethystora-bazaar-line'}));
    list.add_child(data);
    dialog.contentLayout.add_child(list);

    const note = new St.Label({style_class: 'amethystora-bazaar-choice-detail', x_align: Clutter.ActorAlign.CENTER});
    note.clutter_text.line_wrap = true;
    note.text = _('Documents saved outside its own folder stay.');
    dialog.contentLayout.add_child(note);

    let none = false;
    const answer = new Promise(resolve => {
        let chosen = null;
        dialog.addButton({label: _('Keep'), action: () => dialog.close(), key: Clutter.KEY_Escape});
        const remove = dialog.addButton({
            label: _('Remove'),
            action: () => {
                chosen = {data: data.checked, none};
                dialog.close();
            },
        });
        remove.add_style_class_name('amethystora-bazaar-uninstall');
        dialog.setInitialKeyFocus(data);
        dialog.connect('closed', () => resolve(chosen));
        dialog.open();
    });

    const fill = info => {
        if (!list.get_parent())
            return;
        // Nothing could be read: nothing is claimed, and the tick stays what the person left it
        if (!info) {
            app.detail.text = _('When it was last opened could not be read');
            data.detail.text = _('Its size could not be read');
            return;
        }
        const when = Number.isInteger(info.last_opened) ? GLib.DateTime.new_from_unix_local(info.last_opened) : null;
        app.detail.text = when
            ? _('Last opened in %s').format(when.format(when.get_year() === GLib.DateTime.new_now_local().get_year() ? '%OB' : '%OB %Y'))
            : _('Not opened lately');
        data.detail.text = info.data
            ? _('%s, what it saved in its own folder').format(GLib.format_size(info.data_bytes ?? 0))
            : _('It has none');
        if (info.data === false) {
            none = true;
            data.reactive = false;
            data.checked = false;
        }
        if (Number.isInteger(info.trash_days)) {
            note.text = ngettext('Documents saved outside its own folder stay. You have %d day to change your mind.',
                'Documents saved outside its own folder stay. You have %d days to change your mind.', info.trash_days).format(info.trash_days);
        } else {
            note.text = _('Documents saved outside its own folder stay. Its data stays in the trash until you empty it.');
        }
    };
    return {dialog, answer, fill};
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
