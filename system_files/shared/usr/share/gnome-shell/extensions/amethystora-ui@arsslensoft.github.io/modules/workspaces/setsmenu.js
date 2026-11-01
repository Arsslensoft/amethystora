// The Sets part of the workspace menu: Save this workspace, and under it the saved sets, newest first, each with the
// icons of its first apps, its name and its apps. A set opens with a click (or Enter, or Space); the cross that shows
// on a set under the pointer or the keyboard forgets it. The rows exist only while the menu is open.

import Clutter from 'gi://Clutter';
import Shell from 'gi://Shell';
import St from 'gi://St';

import {gettext as _, ngettext} from 'resource:///org/gnome/shell/extensions/extension.js';
import * as PopupMenu from 'resource:///org/gnome/shell/ui/popupMenu.js';

// The icons of a set's first apps, at the board's size
const ICONS = 4;
const ICON_SIZE = 20;
const FORGET_SIZE = 16;
const MISSING_ICON = 'application-x-executable-symbolic';
const MISSING_OPACITY = 128;

function plainLabel(text, styleClass) {
    const label = new St.Label({text, style_class: styleClass, y_align: Clutter.ActorAlign.CENTER});
    // Names and apps are text, whoever wrote them
    label.clutter_text.use_markup = false;
    return label;
}

export class SetsMenu {
    // module: the workspaces module (its `sets`, openSet()); menu: the workspace menu
    constructor(module, menu) {
        this._module = module;
        this._menu = menu;
        this._items = [];
        this._rows = null;
        this._touch = false;
        this._workspace = null;

        const separator = new PopupMenu.PopupSeparatorMenuItem();
        this._add(separator);

        const saveItem = new PopupMenu.PopupBaseMenuItem({reactive: false, can_focus: false});
        this._save = new St.Button({
            style_class: 'amethystora-workspaces-save',
            can_focus: true,
            x_align: Clutter.ActorAlign.START,
            y_align: Clutter.ActorAlign.CENTER,
        });
        this._saveLabel = plainLabel('', 'amethystora-workspaces-save-label');
        this._save.set_child(this._saveLabel);
        this._save.label_actor = this._saveLabel;
        this._save.connect('clicked', () => this._saved());
        saveItem.add_child(this._save);
        this._add(saveItem);

        this._listItem = new PopupMenu.PopupBaseMenuItem({reactive: false, can_focus: false});
        this._list = new St.BoxLayout({
            style_class: 'amethystora-workspaces-sets',
            orientation: Clutter.Orientation.VERTICAL,
            x_expand: true,
        });
        this._heading = plainLabel(_('Sets'), 'amethystora-workspaces-sets-heading');
        this._list.add_child(this._heading);
        this._listItem.add_child(this._list);
        this._add(this._listItem);
        this._listItem.hide();

        // The rows go once the menu is out of sight, not while it fades
        this._menu.actor.connectObject('hide', () => this._clear(), this);
    }

    _add(item) {
        this._menu.addMenuItem(item);
        this._items.push(item);
    }

    // Where in the menu the part begins, for an item that belongs before it
    get position() {
        return this._menu._getMenuItems().indexOf(this._items[0]);
    }

    // The menu opened on `workspace`; touch: it was opened with a finger, which has no hover to show the crosses
    opened(workspace, touch) {
        this._workspace = workspace;
        this._touch = touch;
        const sets = this._module.sets;
        const can = !!workspace && !!sets && sets.windowsOf(workspace).length > 0;
        this._saveLabel.text = can ? _('Save this workspace') : _('Nothing to save here');
        this._save.reactive = can;
        this._save.can_focus = can;
        this._save.checked = false;
        this._fill();
    }

    _saved() {
        this._module.nameNow();
        const result = this._module.sets?.save(this._workspace);
        if (!result) {
            // None of its windows could be kept after all
            this._saveLabel.text = _('Nothing to save here');
            this._save.checked = false;
            this._save.reactive = false;
            this._save.can_focus = false;
            // The keyboard does not rest on a button that does nothing
            if (this._save.has_key_focus())
                this._menu.actor.grab_key_focus();
            return;
        }
        this._saveLabel.text = result.left > 0
            ? ngettext('Saved as %s (%d window)', 'Saved as %s (%d windows)', result.count).format(result.name, result.count)
            : _('Saved as %s').format(result.name);
        this._save.checked = true;
        this._fill();
    }

    _clear() {
        this._rows?.destroy();
        this._rows = null;
    }

    _fill() {
        this._clear();
        const sets = this._module.sets?.list ?? [];
        this._listItem.visible = sets.length > 0;
        if (!sets.length)
            return;
        this._rows = new St.BoxLayout({style_class: 'amethystora-workspaces-sets-rows', orientation: Clutter.Orientation.VERTICAL});
        for (const set of sets)
            this._rows.add_child(this._row(set));
        this._list.add_child(this._rows);
    }

    _row(set) {
        const appSystem = Shell.AppSystem.get_default();
        const ids = [...new Set(set.windows.map(w => w.app))];
        const apps = ids.map(id => ({id, app: appSystem.lookup_app(id)}));
        const names = apps.map(({id, app}) => app?.get_name() ?? id.replace(/\.desktop$/, ''));
        const described = new Intl.ListFormat(undefined, {style: 'short', type: 'unit'}).format(names);

        const row = new St.BoxLayout({style_class: 'amethystora-workspaces-set', reactive: true, track_hover: true});
        const open = new St.Button({style_class: 'amethystora-workspaces-set-open', can_focus: true, x_expand: true,
            accessible_name: _('Open the set %s').format(set.name)});
        open.accessible_description = described;
        const inside = new St.BoxLayout({style_class: 'amethystora-workspaces-set-inside', x_expand: true});
        const icons = new St.BoxLayout({style_class: 'amethystora-workspaces-set-icons', y_align: Clutter.ActorAlign.CENTER});
        for (const {app} of apps.slice(0, ICONS)) {
            const icon = app ? app.create_icon_texture(ICON_SIZE)
                : new St.Icon({icon_name: MISSING_ICON, icon_size: ICON_SIZE, opacity: MISSING_OPACITY});
            icon.add_style_class_name('amethystora-workspaces-set-icon');
            icons.add_child(icon);
        }
        const text = new St.BoxLayout({orientation: Clutter.Orientation.VERTICAL, x_expand: true,
            y_align: Clutter.ActorAlign.CENTER});
        text.add_child(plainLabel(set.name, 'amethystora-workspaces-set-name'));
        text.add_child(plainLabel(described, 'amethystora-workspaces-set-apps'));
        inside.add_child(icons);
        inside.add_child(text);
        open.set_child(inside);
        open.connect('clicked', () => this._module.openSet(set.name));
        row.add_child(open);

        const forget = new St.Button({
            style_class: 'amethystora-workspaces-set-forget',
            can_focus: true,
            y_align: Clutter.ActorAlign.CENTER,
            accessible_name: _('Forget the set %s').format(set.name),
            child: new St.Icon({icon_name: 'window-close-symbolic', icon_size: FORGET_SIZE}),
        });
        forget.connect('clicked', () => {
            const giveBack = open.has_key_focus() || forget.has_key_focus();
            this._module.sets?.forget(set.name);
            row.destroy();
            if (!this._rows?.get_n_children()) {
                this._clear();
                this._listItem.hide();
            }
            // The keyboard was on the row that went: it goes to Save, or the menu
            if (giveBack && this._save.can_focus)
                this._save.grab_key_focus();
            else if (giveBack)
                this._menu.actor.grab_key_focus();
        });
        row.add_child(forget);

        // The cross shows under the pointer and with the keyboard on the row, and always after a finger opened the menu;
        // the row is ringed while the keyboard is on it
        const sync = () => {
            const shown = this._touch || row.hover || open.has_key_focus() || forget.has_key_focus();
            if (open.has_key_focus())
                row.add_style_class_name('amethystora-workspaces-set-focused');
            else
                row.remove_style_class_name('amethystora-workspaces-set-focused');
            forget.opacity = shown ? 255 : 0;
            forget.reactive = shown;
        };
        row.connect('notify::hover', sync);
        for (const actor of [open, forget]) {
            actor.connect('key-focus-in', sync);
            actor.connect('key-focus-out', sync);
        }
        sync();
        return row;
    }

    destroy() {
        this._menu.actor?.disconnectObject(this);
        this._clear();
        for (const item of this._items)
            item.destroy();
        this._items = [];
    }
}
