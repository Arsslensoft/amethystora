// A plugin as a tile of quick settings (R7-07): a switch and a value. The title is the plugin's name, the subtitle what it printed;
// the tile is on while the first `switch` of its card is, and a click asks the host for that switch's action (or, with none, the
// plugin's `click`). Quick settings adopts it by its _amethystoraQuickId, like any other tile, so it can be moved and put away.

import Gio from 'gi://Gio';

import {gettext as _} from 'resource:///org/gnome/shell/extensions/extension.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import {QuickToggle} from 'resource:///org/gnome/shell/ui/quickSettings.js';

// A tile's id is [a-z0-9-]{1,40}: the plugin's, with its dots as hyphens, and a short sum so that cutting it short never makes two the same
export function tileName(id) {
    let sum = 0;
    for (const c of id)
        sum = (sum * 31 + c.charCodeAt(0)) >>> 0;
    return `plugin-${id.replace(/[^a-z0-9]+/g, '-').slice(0, 24)}-${sum.toString(16).slice(0, 7)}`.slice(0, 40);
}

export class PluginTile {
    constructor(plugin, ui) {
        this._ui = ui;
        this.tile = new QuickToggle({title: plugin.name, iconName: 'applications-system-symbolic', toggleMode: false});
        this.tile._amethystoraQuickId = tileName(plugin.id);
        this.tile.connect('clicked', () => {
            // A tile of a plugin that stopped itself resumes it
            if (this.plugin.paused) {
                this._ui.resume({key: this.plugin.id});
                return;
            }
            const out = this.plugin.output ?? this.plugin.sample;
            const action = out?.card.find(e => e.type === 'switch' && e.action)?.action ?? out?.click;
            if (action)
                this._ui.act({key: this.plugin.id}, action);
            else
                Main.panel.closeQuickSettings();
        });
        this.set(plugin);
    }

    set(plugin) {
        this.plugin = plugin;
        const out = plugin.output ?? plugin.sample;
        this.tile.title = plugin.draft ? `${_('Draft')} · ${plugin.name}` : plugin.name;
        // Hidden is for the pill, the bar and the desktop; a tile was asked for, so it stays
        this.tile.visible = Boolean(out) && !this._ui.hidden({output: out});
        if (!out)
            return;
        this.tile.subtitle = plugin.paused ? `${this._ui.pausedWords(plugin)}. ${_('Press to resume')}` : this._ui.hidden({output: out}) ? _('Private') : out.text;
        this.tile.gicon = new Gio.ThemedIcon({name: out.icon || 'applications-system-symbolic'});
        this.tile.checked = out.card.find(e => e.type === 'switch')?.on === true;
    }

    destroy() {
        this.tile.destroy();
    }
}
