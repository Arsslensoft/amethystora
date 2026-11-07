// Plugins in Today, the calendar menu's column (plans/done/PLAN-plugins.md, "What everything reaches"): one section under GNOME's
// events, a card for each plugin that contributes `today`, drawn from the same fixed elements as the pill's card (card.js) with
// the plugin's name over it. Built only while such a plugin is on, and gone with the last of them.

import Clutter from 'gi://Clutter';
import St from 'gi://St';

import * as Main from 'resource:///org/gnome/shell/ui/main.js';

import {elements, label} from './card.js';

export class TodayPlugins {
    constructor(ui) {
        this._ui = ui;
        this._section = null;
    }

    // plugins: [{plugin, output}], output already the one to show
    sync(items) {
        const events = Main.panel.statusArea.dateMenu?._eventsItem;
        const displays = events?.get_parent();
        if (!items.length || !displays) {
            this.destroy();
            return;
        }
        if (!this._section) {
            this._section = new St.BoxLayout({orientation: Clutter.Orientation.VERTICAL, x_expand: true,
                style_class: 'amethystora-today-section amethystora-today-plugins'});
            displays.insert_child_above(this._section, events);
        }
        this._section.destroy_all_children();
        for (const {plugin, output} of items) {
            const card = new St.BoxLayout({orientation: Clutter.Orientation.VERTICAL, x_expand: true, style_class: 'amethystora-today-plugin'});
            card.add_child(label(plugin.name, 'amethystora-today-heading'));
            if (this._ui.hiddenOutput(output)) {
                this._section.add_child(card);
                continue;
            }
            if (output.text)
                card.add_child(label(output.text));
            if (output.card.length)
                card.add_child(elements(output.card, action => this._ui.act({key: plugin.id}, action)));
            this._section.add_child(card);
        }
    }

    destroy() {
        this._section?.destroy();
        this._section = null;
    }
}
