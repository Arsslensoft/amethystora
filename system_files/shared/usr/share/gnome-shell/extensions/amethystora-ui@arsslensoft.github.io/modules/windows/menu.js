// The window menu's own lines: GNOME's menu, as GNOME opens it, with the lines of the windows module's parts that want
// one (a copy kept in view, the scratchpad, a size for recording) above its last separator, which comes before Close.
// One wrap of GNOME's one menu manager, on its instance, so that the tiling's wrap of the menu's class (its own line at
// the top) and this never stack on each other.

import GLib from 'gi://GLib';

import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import {WindowMenu} from 'resource:///org/gnome/shell/ui/windowMenu.js';
import {PopupSeparatorMenuItem} from 'resource:///org/gnome/shell/ui/popupMenu.js';

export class MenuHook {
    // lines(menu, window) adds the parts' lines to a menu GNOME has just built, and returns them in order
    constructor(lines) {
        this._lines = lines;
    }

    enable() {
        const hook = this;
        const manager = Main.wm._windowMenuManager;
        const original = manager.showWindowMenuForWindow;
        this._manager = manager;
        this._ownMethod = Object.getOwnPropertyDescriptor(manager, 'showWindowMenuForWindow') ?? null;
        manager.showWindowMenuForWindow = function (window, type, rect) {
            const before = this._manager?.activeMenu ?? null;
            original.call(this, window, type, rect);
            const menu = this._manager?.activeMenu;
            if (!menu || menu === before || !(menu instanceof WindowMenu))
                return;
            // GNOME destroys this menu as it closes, in the click on one of its lines, whose press then ends on a line
            // that is gone: the menu goes just after that event instead
            const destroy = menu.destroy;
            menu.destroy = () => GLib.idle_add(GLib.PRIORITY_HIGH, () => {
                destroy.call(menu);
                return GLib.SOURCE_REMOVE;
            });
            try {
                hook._add(menu, window);
            } catch (e) {
                console.error(`Amethystora UI: the window menu's lines were not added: ${e.message}`);
            }
        };
    }

    disable() {
        // GNOME's own method again: the class's, which the instance only covered
        if (this._manager) {
            if (this._ownMethod)
                Object.defineProperty(this._manager, 'showWindowMenuForWindow', this._ownMethod);
            else
                delete this._manager.showWindowMenuForWindow;
        }
        this._manager = null;
        this._ownMethod = null;
    }

    _add(menu, window) {
        const items = this._lines(menu, window).filter(item => item);
        if (!items.length)
            return;
        let separator = -1;
        menu._getMenuItems().forEach((each, i) => {
            if (each instanceof PopupSeparatorMenuItem)
                separator = i;
        });
        if (separator < 0)
            return;
        // Each goes just before the last separator, in the order given
        for (const item of items) {
            menu.moveMenuItem(item, separator);
            separator++;
        }
    }
}
