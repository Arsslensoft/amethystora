// Plugins in the logo menu (plans/PLAN-plugins.md, "What everything reaches"): a section at the end of Amethystora's menu, one line
// for each plugin that contributes `menu`, its name and what it printed, a click running its `click` action in the sandbox as any
// click does. The menu is the logo menu extension's; this only adds a section to it while such a plugin is on, and takes it away.

import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import * as PopupMenu from 'resource:///org/gnome/shell/ui/popupMenu.js';

const LOGO_MENU = 'amethystora-logo-menu@arsslensoft.github.io';

export class MenuPlugins {
    constructor(ui) {
        this._ui = ui;
        this._section = null;
        this._separator = null;
    }

    sync(items) {
        const menu = Main.panel.statusArea[LOGO_MENU]?.menu;
        if (!items.length || !menu) {
            this.destroy();
            return;
        }
        if (this._menu !== menu)
            this.destroy();
        if (!this._section) {
            this._menu = menu;
            this._separator = new PopupMenu.PopupSeparatorMenuItem();
            this._section = new PopupMenu.PopupMenuSection();
            menu.addMenuItem(this._separator);
            menu.addMenuItem(this._section);
        }
        this._section.removeAll();
        for (const {plugin, output} of items) {
            const words = this._ui.hiddenOutput(output) || !output.text ? plugin.name : `${plugin.name}: ${output.text}`;
            const item = new PopupMenu.PopupMenuItem(words.slice(0, 120));
            item.label.clutter_text.use_markup = false;
            if (output.click)
                item.connect('activate', () => this._ui.act({key: plugin.id}, output.click));
            else
                item.setSensitive(false);
            this._section.addMenuItem(item);
        }
    }

    destroy() {
        this._separator?.destroy();
        this._section?.destroy();
        this._separator = null;
        this._section = null;
        this._menu = null;
    }
}
