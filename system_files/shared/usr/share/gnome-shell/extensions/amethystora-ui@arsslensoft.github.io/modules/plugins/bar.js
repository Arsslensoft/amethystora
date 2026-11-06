// A plugin in the top bar: a label, an icon and a menu, drawn from what the host passed (the widgets' own look, which these replace).
// The menu ends with what the shell says of it, whatever the plugin printed: its name, what it may reach and Pause. Nothing runs in
// the shell: a menu item asks the host to run one of the plugin's own actions, in its sandbox.

import Clutter from 'gi://Clutter';
import GObject from 'gi://GObject';
import Gio from 'gi://Gio';
import Pango from 'gi://Pango';
import St from 'gi://St';

import {gettext as _} from 'resource:///org/gnome/shell/extensions/extension.js';
import * as PanelMenu from 'resource:///org/gnome/shell/ui/panelMenu.js';
import * as PopupMenu from 'resource:///org/gnome/shell/ui/popupMenu.js';

import {reachWords} from './card.js';
import {reach} from './clamp.js';

const CLASS_COLORS = {accent: 'accent', good: 'color2', warning: 'color3', critical: 'color1'};
const EMPTY = {text: '', icon: '', class: '', click: null, hidden: false, private: false, progress: null, until: null, menu: [], card: []};

function inert(text) {
    const item = new PopupMenu.PopupMenuItem(text, {reactive: false, can_focus: false});
    item.remove_style_class_name('popup-inactive-menu-item');
    item.label.clutter_text.use_markup = false;
    item.label.clutter_text.ellipsize = Pango.EllipsizeMode.NONE;
    item.label.clutter_text.line_wrap = true;
    return item;
}

export const BarButton = GObject.registerClass(
class AmethystoraPluginButton extends PanelMenu.Button {
    _init(plugin, ui) {
        super._init(0.5, plugin.name);
        this._ui = ui;
        this._key = null;
        this.plugin = plugin;
        this._box = new St.BoxLayout({style_class: 'panel-status-menu-box'});
        this._icon = new St.Icon({style_class: 'system-status-icon', visible: false});
        this._label = new St.Label({style_class: 'amethystora-widget-label', y_align: Clutter.ActorAlign.CENTER, visible: false});
        this._label.clutter_text.use_markup = false;
        this._label.clutter_text.ellipsize = Pango.EllipsizeMode.END;
        this._box.add_child(this._icon);
        this._box.add_child(this._label);
        this.add_child(this._box);
        this.visible = false;
        this.set(plugin);
    }

    get side() {
        return this.plugin.places.bar?.position ?? 'right';
    }

    get rank() {
        return this.plugin.places.bar?.order ?? 0;
    }

    // A new state of the same plugin: drawn again only when something it shows changed
    set(plugin) {
        this.plugin = plugin;
        const shown = plugin.output ?? plugin.sample;
        // A plugin that failed or paused with nothing to show is a warning sign with a menu that says why and can resume it
        const failed = !shown && (plugin.paused || plugin.error !== '');
        const out = shown ?? EMPTY;
        this.visible = failed || (Boolean(shown) && !shown.hidden);
        if (!shown && !failed)
            return;
        const hidden = !failed && this._ui.hidden({output: out});
        const symbol = failed ? 'dialog-warning-symbolic' : hidden ? '' : out.icon;
        this._icon.visible = symbol !== '';
        if (symbol)
            this._icon.gicon = new Gio.ThemedIcon({name: symbol});
        // Its name beside its words: whatever it prints is drawn as its own
        const words = failed ? '' : [plugin.draft ? `${_('Draft')} ${plugin.name}` : plugin.name, hidden ? _('Private') : out.text].filter(Boolean).join(' · ');
        this._label.visible = words !== '';
        this._label.text = words;
        const colour = this._ui.palette[plugin.paused || failed ? 'color3' : CLASS_COLORS[out.class]];
        this._box.style = colour ? `color: ${colour};` : null;
        // While the screen is shared a private plugin's menu and card say nothing
        const drawn = hidden ? EMPTY : out;
        const key = JSON.stringify([drawn.menu, drawn.card, plugin.paused, plugin.reason, plugin.error, plugin.granted, plugin.waiting]);
        if (key !== this._key) {
            this._key = key;
            this._fill(drawn);
        }
    }

    _fill(out) {
        const plugin = this.plugin;
        this.menu.removeAll();
        if (plugin.paused || plugin.waiting || plugin.error) {
            this.menu.addMenuItem(inert(plugin.paused || plugin.waiting ? this._ui.pausedWords(plugin) : plugin.error));
            this.menu.addMenuItem(new PopupMenu.PopupSeparatorMenuItem());
        }
        for (const item of out.menu) {
            if (item.separator) {
                this.menu.addMenuItem(new PopupMenu.PopupSeparatorMenuItem());
                continue;
            }
            const entry = item.icon ? new PopupMenu.PopupImageMenuItem(item.label, new Gio.ThemedIcon({name: item.icon}), item.action ? {} : {reactive: false, can_focus: false})
                : new PopupMenu.PopupMenuItem(item.label, item.action ? {} : {reactive: false, can_focus: false});
            entry.label.clutter_text.use_markup = false;
            if (item.action)
                entry.connect('activate', () => this._ui.act({key: plugin.id}, item.action));
            this.menu.addMenuItem(entry);
        }
        for (const e of out.card) {
            const text = e.type === 'row' ? `${e.label}: ${e.value}` : e.type === 'list' ? e.items.join(', ') : e.text ?? e.label;
            if (e.type === 'button' && e.action) {
                const button = new PopupMenu.PopupMenuItem(e.label);
                button.connect('activate', () => this._ui.act({key: plugin.id}, e.action));
                this.menu.addMenuItem(button);
            } else if (text) {
                this.menu.addMenuItem(inert(text));
            }
        }
        // What the shell says of it
        this.menu.addMenuItem(new PopupMenu.PopupSeparatorMenuItem());
        this.menu.addMenuItem(inert(`${plugin.name}${plugin.draft ? ` · ${_('Draft')}` : ''}`));
        this.menu.addMenuItem(inert(reachWords(reach(plugin.granted))));
        const pause = new PopupMenu.PopupMenuItem(plugin.paused ? _('Resume') : _('Pause'));
        pause.connect('activate', () => (plugin.paused ? this._ui.resume({key: plugin.id}) : this._ui.pause({key: plugin.id})));
        this.menu.addMenuItem(pause);
    }

    // A plugin with a "click" and an empty menu runs it, instead of opening a menu with only what the shell says
    // A secondary press always opens the menu, which holds the shell's own lines (its name, what it may reach, Pause)
    vfunc_event(event) {
        const type = event.type();
        const out = this.plugin.output;
        if (out?.click && out.menu.length === 0 && out.card.length === 0 && event.get_button?.() !== 3 &&
            (type === Clutter.EventType.BUTTON_PRESS || type === Clutter.EventType.TOUCH_BEGIN)) {
            this._ui.act({key: this.plugin.id}, out.click);
            return Clutter.EVENT_STOP;
        }
        return super.vfunc_event(event);
    }
});
