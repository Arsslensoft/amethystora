// The dock's items that are not apps (a drive, the trash, a stack, a folder) and their menus, laid out as GNOME's
// app menu is: the place's name, its windows, a new window, what can be done with the place, and closing its windows.
// GNOME's app menu, wherever it opens, pins to the dock and says how many windows Quit closes; on the dock it also has
// the app's name at its top and the app's own entries, which it gives through the launcher protocol. While an app is
// being updated its menu says so at the top and leaves out everything that would start it.

import Clutter from 'gi://Clutter';
import GObject from 'gi://GObject';
import St from 'gi://St';

import {gettext as _, ngettext} from 'resource:///org/gnome/shell/extensions/extension.js';
import * as AppMenu from 'resource:///org/gnome/shell/ui/appMenu.js';
import * as BoxPointer from 'resource:///org/gnome/shell/ui/boxpointer.js';
import * as Dash from 'resource:///org/gnome/shell/ui/dash.js';
import * as IconGrid from 'resource:///org/gnome/shell/ui/iconGrid.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import * as PopupMenu from 'resource:///org/gnome/shell/ui/popupMenu.js';

// The name over a menu, which is no item to choose
function heading(text) {
    const item = new PopupMenu.PopupMenuItem(text, {reactive: false, can_focus: false});
    item.add_style_class_name('amethystora-dock-menu-heading');
    return item;
}

// The left and middle buttons, under the names GNOME 51 gives them or the ones 50 has
const BUTTONS = (St.ButtonMask.PRIMARY ?? St.ButtonMask.ONE) | (St.ButtonMask.MIDDLE ?? St.ButtonMask.TWO);

// GNOME's dash takes this item for an icon (it has a delegate with an icon), so it is sized with the apps, but not
// for an app, so GNOME's list of apps leaves it alone. activate(button) is a click; fill(menu) fills its menu.
export const PlaceButton = GObject.registerClass({
    Signals: {'menu-state-changed': {param_types: [GObject.TYPE_BOOLEAN]}},
}, class AmethystoraDockPlace extends St.Button {
    _init(place, activate, fill) {
        super._init({
            style_class: 'overview-tile',
            reactive: true,
            can_focus: true,
            track_hover: true,
            button_mask: BUTTONS,
        });
        this.place = place;
        this._activate = activate;
        this._fill = fill;
        this._delegate = this;

        this._iconContainer = new St.Widget({layout_manager: new Clutter.BinLayout(), x_expand: true, y_expand: true});
        this.set_child(this._iconContainer);
        this.icon = new IconGrid.BaseIcon(place.name, {
            setSizeManually: true,
            showLabel: false,
            // A folder of apps draws its own picture
            createIcon: size => this.place.createIcon?.(size) ?? new St.Icon({
                gicon: this.place.gicon,
                fallback_icon_name: this.place.fallback ?? 'folder',
                icon_size: size,
            }),
        });
        this._iconContainer.add_child(this.icon);

        const longPress = new Clutter.LongPressGesture();
        longPress.connect('recognize', () => this.popupMenu());
        this.add_action(longPress);
        const rightClick = new Clutter.ClickGesture({
            required_button: Clutter.BUTTON_SECONDARY,
            recognize_on_press: true,
        });
        rightClick.connect('recognize', () => this.popupMenu());
        this.add_action(rightClick);
        this.connect('popup-menu', () => {
            this.popupMenu();
            this._menu.actor.navigate_focus(null, St.DirectionType.TAB_FORWARD, false);
        });
        this.connect('destroy', () => this._menu?.destroy());
    }

    vfunc_clicked(button) {
        this._activate(button);
    }

    // The place's icon changed (the trash filled or emptied, a drive was renamed, a folder's colour)
    sync() {
        if (this.icon.icon)
            this.icon._createIconTexture(this.icon.iconSize);
    }

    popupMenu() {
        if (!this._menu) {
            this._menu = new PopupMenu.PopupMenu(this, 0.5, St.Side.BOTTOM);
            this._menu.actor.add_style_class_name('app-menu');
            this._menu.connect('open-state-changed', (menu, open) => {
                if (!open) {
                    this.track_hover = true;
                    this.sync_hover();
                }
                this.emit('menu-state-changed', open);
            });
            Main.overview.connectObject('hiding', () => this._menu.close(), this);
            Main.uiGroup.add_child(this._menu.actor);
            this._menuManager = new PopupMenu.PopupMenuManager(this);
            this._menuManager.addMenu(this._menu);
        }
        this._fill(this._menu);
        // Lit while its menu is open, as GNOME's app icons are
        this.set({track_hover: false, hover: true});
        this._menu.open(BoxPointer.PopupAnimation.FULL);
        return false;
    }
});

// windows() gives the place's windows, most recently used first, as they are when asked
export function fillPlaceMenu(menu, place, places, windows) {
    menu.removeAll();
    menu.addMenuItem(heading(place.name));
    const shown = windows();
    if (shown.length) {
        menu.addMenuItem(new PopupMenu.PopupSeparatorMenuItem(_('Open windows')));
        for (const window of shown) {
            const item = menu.addAction(window.title || place.name,
                () => Main.activateWindow(window, global.get_current_time()));
            window.connectObject('notify::title', () => (item.label.text = window.title || place.name), item);
        }
        menu.addMenuItem(new PopupMenu.PopupSeparatorMenuItem());
    }
    menu.addAction(_('New window'), () => {
        places.openNew(place);
        Main.overview.hide();
    });
    menu.addMenuItem(new PopupMenu.PopupSeparatorMenuItem());
    for (const action of places.actions(place))
        menu.addAction(action.label, () => action.run());
    if (shown.length) {
        menu.addMenuItem(new PopupMenu.PopupSeparatorMenuItem());
        menu.addAction(ngettext('Close the window', 'Close %d windows', shown.length).format(shown.length), () => {
            for (const window of windows())
                window.delete(global.get_current_time());
        });
    }
}

// An app's own entries ([{label, enabled, separator, activate(time)}]) after the app's actions (and GNOME's
// discrete GPU item), right before GNOME's line over the pin item (else before the details item)
function fillQuicklist(menu, entries) {
    if (!menu._amethystoraQuicklist && !entries.length)
        return;
    if (!menu._amethystoraQuicklist) {
        menu._amethystoraQuicklist = new PopupMenu.PopupMenuSection();
        const items = menu._getMenuItems();
        const pin = items.indexOf(menu._toggleFavoriteItem);
        const details = items.indexOf(menu._detailsItem);
        const line = pin > 0 && items[pin - 1] instanceof PopupMenu.PopupSeparatorMenuItem ? pin - 1 : pin;
        menu.addMenuItem(menu._amethystoraQuicklist, line >= 0 ? line : details >= 0 ? details : items.length);
    }
    const section = menu._amethystoraQuicklist;
    section.removeAll();
    for (const entry of entries) {
        if (entry.separator) {
            section.addMenuItem(new PopupMenu.PopupSeparatorMenuItem());
            continue;
        }
        const item = section.addAction(entry.label, () => entry.activate(global.get_current_time()));
        item.setSensitive(entry.enabled);
    }
    // GNOME shows or hides its lines as a menu opens, which this one already has
    for (const item of menu._getMenuItems()) {
        if (item instanceof PopupMenu.PopupSeparatorMenuItem)
            menu._updateSeparatorVisibility(item);
    }
}

// GNOME's app menus, wherever they open, while the dock runs
export class AppMenus {
    constructor(launcher) {
        this._launcher = launcher;
    }

    enable() {
        this._menus = new Set();
        this._wrapped = [];
        const menus = this;
        const proto = AppMenu.AppMenu.prototype;
        this._wrap(proto, '_updateFavoriteItem', function (original, args) {
            original.apply(this, args);
            menus._track(this);
            if (this._toggleFavoriteItem.visible && this._app && !this._appFavorites.isFavorite(this._app.get_id()))
                this._toggleFavoriteItem.label.text = _('Pin to dock');
        });
        this._wrap(proto, '_updateQuitItem', function (original, args) {
            original.apply(this, args);
            menus._labelQuit(this);
        });
        // GNOME lists the windows again whenever the app's windows change
        this._wrap(proto, '_updateWindowsSection', function (original, args) {
            original.apply(this, args);
            menus._labelQuit(this);
        });
        this._wrap(proto, 'open', function (original, args) {
            menus._prepare(this);
            return original.apply(this, args);
        });
    }

    disable() {
        for (const [proto, name, descriptor] of [...this._wrapped ?? []].reverse()) {
            if (descriptor)
                Object.defineProperty(proto, name, descriptor);
            else
                delete proto[name];
        }
        this._wrapped = null;
        // The menus GNOME keeps (the app grid's) are as GNOME made them again
        for (const menu of this._menus ?? []) {
            menu.actor.disconnectObject(this);
            menu._amethystoraHeading?.destroy();
            menu._amethystoraQuicklist?.destroy();
            menu._amethystoraHeading = menu._amethystoraQuicklist = null;
            if (menu._amethystoraQuit !== undefined)
                menu._quitItem.label.text = menu._amethystoraQuit;
            delete menu._amethystoraQuit;
            menu._actionSection.actor.show();
            menu._updateNewWindowItem();
            menu._updateGpuItem();
            menu._updateFavoriteItem();
        }
        this._menus = null;
    }

    _wrap(proto, name, around) {
        const original = proto[name];
        this._wrapped.push([proto, name, Object.getOwnPropertyDescriptor(proto, name)]);
        proto[name] = function (...args) {
            return around.call(this, original, args);
        };
    }

    _track(menu) {
        if (!this._menus || this._menus.has(menu))
            return;
        this._menus.add(menu);
        menu.actor.connectObject('destroy', () => this._menus?.delete(menu), this);
    }

    // Quit says how many windows it closes, when that is more than one
    _labelQuit(menu) {
        this._track(menu);
        const label = menu._quitItem.label;
        if (menu._amethystoraQuit === undefined)
            menu._amethystoraQuit = label.text;
        const count = menu._app?.get_windows().filter(window => !window.skip_taskbar).length ?? 0;
        label.text = count > 1 ? ngettext('Quit %d window', 'Quit %d windows', count).format(count)
            : menu._amethystoraQuit;
    }

    // Just before the menu opens: the heading, the app's own entries, and what an update leaves out
    _prepare(menu) {
        const app = menu._app;
        if (!app)
            return;
        this._track(menu);
        this._labelQuit(menu);
        const updating = Boolean(this._launcher.get(app.get_id())?.updating);
        const dock = menu.sourceActor instanceof Dash.DashIcon;

        if (dock || updating) {
            if (!menu._amethystoraHeading) {
                menu._amethystoraHeading = heading('');
                menu.addMenuItem(menu._amethystoraHeading, 0);
            }
            menu._amethystoraHeading.label.text = updating
                ? _('%s is being updated').format(app.get_name()) : app.get_name();
            menu._amethystoraHeading.show();
        } else {
            menu._amethystoraHeading?.hide();
        }

        fillQuicklist(menu, dock && !updating ? this._launcher.quicklist(app.get_id()) : []);

        // Nothing that would start the app: a new window, the other graphics card, its actions
        if (updating) {
            menu._newWindowItem.hide();
            menu._onGpuMenuItem.hide();
            menu._actionSection.actor.hide();
        } else {
            menu._updateNewWindowItem();
            menu._updateGpuItem();
            menu._actionSection.actor.show();
        }
    }
}
