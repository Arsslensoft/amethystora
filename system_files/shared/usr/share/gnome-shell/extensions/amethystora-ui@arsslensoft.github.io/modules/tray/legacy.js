// The old X11 system tray, for programs that dock an icon window rather than publish an item: GNOME Shell's own
// tray manager does the X11 work under Xwayland, and each icon it hands over gets a button beside the others. The
// program draws its own popup, so a click on the button is passed to its icon as a click. Under Wayland without
// Xwayland (`ame security block-xwayland`) there is no X11 tray, and nothing here does anything.

import Clutter from 'gi://Clutter';
import GObject from 'gi://GObject';
import Shell from 'gi://Shell';
import St from 'gi://St';

import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import * as PanelMenu from 'resource:///org/gnome/shell/ui/panelMenu.js';

import {ICON_SIZE} from './button.js';

const LegacyButton = GObject.registerClass(
class LegacyButton extends PanelMenu.Button {
    /**
     * @param {Shell.TrayIcon} icon - the docked icon, which the tray manager keeps
     */
    _init(icon) {
        // No menu of its own
        super._init(0.5, icon.wm_class ?? '', true);
        this.add_style_class_name('amethystora-tray-button');
        this.add_style_class_name('amethystora-tray-legacy');
        this.trayIcon = icon;
        this._bin = new St.Bin({
            child: icon,
            x_align: Clutter.ActorAlign.CENTER,
            y_align: Clutter.ActorAlign.CENTER,
        });
        this.add_child(this._bin);

        const context = St.ThemeContext.get_for_stage(global.stage);
        context.connectObject('notify::scale-factor', () => this._resize(), this);
        this._resize();
        // The icon belongs to the tray manager, which lets it go itself
        this.connect('destroy', () => {
            this._bin.child = null;
        });
    }

    _resize() {
        const size = ICON_SIZE * St.ThemeContext.get_for_stage(global.stage).scale_factor;
        this.trayIcon.set_size(size, size);
    }

    vfunc_button_press_event() {
        this.add_style_pseudo_class('active');
        return Clutter.EVENT_STOP;
    }

    vfunc_button_release_event(event) {
        this.remove_style_pseudo_class('active');
        this.trayIcon.click(event);
        return Clutter.EVENT_STOP;
    }

    // A key looks pressed as a button does
    vfunc_key_press_event(event) {
        this.add_style_pseudo_class('active');
        this.trayIcon.click(event);
        return Clutter.EVENT_STOP;
    }

    vfunc_key_release_event(event) {
        this.remove_style_pseudo_class('active');
        this.trayIcon.click(event);
        return Clutter.EVENT_STOP;
    }
});

export class LegacyTray {
    /**
     * @param {function(PanelMenu.Button): void} place - puts a button in the top bar
     */
    constructor(place) {
        this._place = place;
    }

    enable() {
        this._buttons = new Map();
        // Behind each icon the panel's colour, taken once, as the tray manager wants it when it is made
        let color = null;
        try {
            color = Main.panel.get_theme_node().get_background_color();
        } catch {}
        this._manager = color ? new Shell.TrayManager({bg_color: color}) : new Shell.TrayManager();
        this._manager.connectObject(
            'tray-icon-added', (m, icon) => this._added(icon),
            'tray-icon-removed', (m, icon) => this._removed(icon), this);
        this._manager.manage_screen(Main.panel);
    }

    disable() {
        for (const button of this._buttons?.values() ?? [])
            button.destroy();
        this._buttons = null;
        if (this._manager) {
            this._manager.disconnectObject(this);
            this._manager.unmanage_screen();
        }
        this._manager = null;
    }

    _added(icon) {
        const button = new LegacyButton(icon);
        this._buttons.set(icon, button);
        this._place(button);
    }

    _removed(icon) {
        this._buttons.get(icon)?.destroy();
        this._buttons.delete(icon);
    }
}
