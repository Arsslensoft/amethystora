// Tray icons in the top bar. A program that shows a status icon, through the StatusNotifierItem protocol (Qt and KDE
// programs, Electron apps, anything built with libappindicator or Ayatana) or the old X11 system tray, gets a button
// in the right of the top bar with its icon, its label and its own menu. The shell is the session's tray for this:
// see modules/tray/watcher.js. Icons come and go as their programs start, change and stop, and since GNOME takes the
// extension away while the screen is locked, they come back after every unlock: programs that do not register again
// by themselves are found two seconds later.

import GLib from 'gi://GLib';
import Gio from 'gi://Gio';
import St from 'gi://St';

import * as Main from 'resource:///org/gnome/shell/ui/main.js';

import {ItemButton} from './tray/button.js';
import {IconFinder} from './tray/icons.js';
import {LegacyTray} from './tray/legacy.js';
import {findItems, serviceFor} from './tray/scan.js';
import {Watcher} from './tray/watcher.js';
import {isCancelled} from './tray/dbus.js';

// Where the buttons go: the right of the top bar, straight after the meters (the network's is the last of them), so
// that a newer icon stands left of an older, and at the start of the box where there are no meters
const BOX = 'right';
const AFTER = 'amethystora-meters-network';
// The status area's names for the buttons
const ROLE = 'amethystora-tray-';
// Milliseconds after the tray starts before programs that did not register are looked for
const SCAN_DELAY = 2000;

let serial = 0;

export class Tray {
    constructor(ext) {
        this._ext = ext;
    }

    enable() {
        // A button left by a tray that did not stop cleanly
        for (const [role, indicator] of Object.entries(Main.panel.statusArea)) {
            if (role.startsWith(ROLE))
                indicator?.destroy();
        }

        this._buttons = new Map();
        this._icons = new IconFinder();
        this._cancellable = new Gio.Cancellable();

        // A change the items did not make, which still changes their pictures
        St.TextureCache.get_default().connectObject('icon-theme-changed', () => this._reloadIcons(), this);
        St.ThemeContext.get_for_stage(global.stage).connectObject('notify::scale-factor',
            () => this._reloadIcons(), this);

        this._watcher = new Watcher({
            add: item => this._place(new ItemButton(item, this._icons), item),
            remove: item => {
                this._buttons.get(item)?.destroy();
                this._buttons.delete(item);
            },
        });
        this._watcher.enable();

        this._legacy = new LegacyTray(button => this._place(button, null));
        this._legacy.enable();

        this._scanId = GLib.timeout_add(GLib.PRIORITY_LOW, SCAN_DELAY, () => {
            this._scanId = 0;
            this._scan();
            return GLib.SOURCE_REMOVE;
        });
    }

    disable() {
        if (this._scanId)
            GLib.source_remove(this._scanId);
        this._scanId = 0;
        this._cancellable?.cancel();
        this._cancellable = null;

        this._legacy?.disable();
        this._legacy = null;
        this._watcher?.disable();
        this._watcher = null;
        for (const button of this._buttons?.values() ?? [])
            button.destroy();
        this._buttons = null;

        St.TextureCache.get_default().disconnectObject(this);
        St.ThemeContext.get_for_stage(global.stage).disconnectObject(this);
        this._icons?.destroy();
        this._icons = null;
    }

    // Asked at every placing, not once: the meters may come after the tray, or go, and each puts itself at the start
    // of the box, so that the buttons placed after them stay behind them
    _index() {
        const meters = Main.panel.statusArea[AFTER];
        return meters ? Main.panel._rightBox.get_children().indexOf(meters.container) + 1 : 0;
    }

    _place(button, item) {
        try {
            Main.panel.addToStatusArea(`${ROLE}${++serial}`, button, this._index(), BOX);
        } catch (e) {
            button.destroy();
            throw e;
        }
        if (item)
            this._buttons.set(item, button);
    }

    _reloadIcons() {
        for (const item of this._buttons.keys())
            item.reloadIcons();
    }

    async _scan() {
        const cancellable = this._cancellable;
        let found;
        try {
            found = await findItems(cancellable);
        } catch (e) {
            if (!isCancelled(e))
                console.warn(`Amethystora UI: the tray could not look for items: ${e.message}`);
            return;
        }
        if (cancellable.is_cancelled())
            return;
        for (const item of found) {
            if (this._watcher.has(item.owner, item.path))
                continue;
            this._watcher.register(serviceFor(item), null).catch(e => {
                if (!cancellable.is_cancelled())
                    console.debug(`Amethystora UI: a tray item found on the bus was not added: ${e.message}`);
            });
        }
    }
}
