// Plugins on the desktop (board 49, R4-19; offered, off). A plugin whose manifest contributes `desktop` is a small glass card on the
// wallpaper of the primary screen, under every window, in the corner it names. It is the same plugin as the bar's and the pill's: the
// host runs it in its sandbox, the `plugins` module (modules/plugins.js) hands this module what it printed, and this module only draws
// it, as lines of plain text; nothing here runs a command. While this is off a desktop plugin is drawn nowhere, and the host still runs
// it only if it is on (ame plugin on).
//
// The cards are a layer of the window group right under the first window: over the wallpaper, a workspace's own
// picture and Focus Sleep's night picture, which sit in the same group, and put back there whenever Mutter restacks
// the windows (a restack lowers only the windows and the background group). Off, only the setting is connected.

import Clutter from 'gi://Clutter';
import Gio from 'gi://Gio';
import Meta from 'gi://Meta';
import Pango from 'gi://Pango';
import St from 'gi://St';

import {gettext as _} from 'resource:///org/gnome/shell/extensions/extension.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import * as PopupMenu from 'resource:///org/gnome/shell/ui/popupMenu.js';

const SCHEMA = 'org.gnome.shell.extensions.amethystora-ui.deskwidgets';
const CLASS_COLORS = {accent: 'accent', good: 'color2', warning: 'color3', critical: 'color1'};
// From the work area's edges, and between two cards in a corner
const EDGE = 16;
const GAP = 12;
// A card shows at most this many lines of a plugin's text, the last cut short
const MOST_LINES = 6;

function textLabel(text, styleClass) {
    const label = new St.Label({text, style_class: styleClass, y_align: Clutter.ActorAlign.CENTER});
    label.clutter_text.use_markup = false;
    label.clutter_text.ellipsize = Pango.EllipsizeMode.END;
    return label;
}

class Card {
    constructor(module, plugin) {
        this._module = module;
        this.plugin = plugin;
        this._items = [];
        this._menuKey = null;
        this.actor = new St.BoxLayout({
            style_class: 'amethystora-deskwidgets-card',
            orientation: Clutter.Orientation.VERTICAL,
            reactive: true,
            track_hover: true,
            can_focus: true,
            accessible_name: plugin.name,
            visible: false,
        });
        const click = new Clutter.ClickGesture();
        click.connect('recognize', () => this._clicked());
        this.actor.add_action(click);
        this.set(plugin);
    }

    get corner() {
        return this.plugin.places.desktop?.corner ?? 'top-right';
    }

    get rank() {
        return 0;
    }

    get output() {
        return this.plugin.output ?? this.plugin.sample;
    }

    // What it says now: its lines, the icon beside the first; a card that says nothing is not drawn
    set(plugin) {
        this.plugin = plugin;
        const out = this.output;
        const colour = this._module.palette[plugin.paused ? 'color3' : CLASS_COLORS[out?.class]] ?? null;
        this.actor.destroy_all_children();
        // A plugin that failed and has nothing to show says so, as the widgets' warning card did
        const failed = !out && plugin.error !== '';
        this.actor.visible = failed || (Boolean(out) && !out.hidden);
        const rows = failed ? [plugin.name, plugin.error] : out ? (plugin.draft ? [_('Draft'), ...out.lines.slice(0, MOST_LINES)] : out.lines.slice(0, MOST_LINES)) : [];
        if (this.actor.visible && rows.length === 0)
            rows.push(plugin.name);
        const warn = failed ? this._module.palette.color3 ?? null : null;
        rows.forEach((line, i) => {
            const label = textLabel(line, i === 0 ? 'amethystora-deskwidgets-title' : 'amethystora-deskwidgets-line');
            if ((colour || warn) && i === rows.length - 1)
                label.style = `color: ${warn ?? colour};`;
            const symbol = failed ? 'dialog-warning-symbolic' : out?.icon;
            if (i === 0 && symbol) {
                const row = new St.BoxLayout({style_class: 'amethystora-deskwidgets-head'});
                row.add_child(new St.Icon({gicon: new Gio.ThemedIcon({name: symbol}), icon_size: 16, y_align: Clutter.ActorAlign.CENTER}));
                row.add_child(label);
                this.actor.add_child(row);
            } else {
                this.actor.add_child(label);
            }
        });
        const items = [...out?.menu ?? []];
        for (const e of out?.card ?? []) {
            if (e.type === 'button' && e.action)
                items.push({label: e.label, action: e.action, icon: ''});
        }
        if (plugin.paused)
            items.unshift({label: this._module.pausedWords(plugin), action: null, icon: ''});
        const key = JSON.stringify(items);
        if (key !== this._menuKey) {
            this._menuKey = key;
            this._items = items;
            if (this._menu?.isOpen)
                this._openMenu();
        }
        this._module.queueLayout();
    }

    _clicked() {
        const out = this.output;
        if (out?.click && this._items.length === 0) {
            this._module.act(this.plugin.id, out.click);
            return;
        }
        if (this._items.length === 0)
            return;
        if (this._menu?.isOpen) {
            this._menu.close();
            return;
        }
        this._openMenu();
    }

    _openMenu() {
        this._closeMenu();
        const top = this.corner.startsWith('top');
        const menu = new PopupMenu.PopupMenu(this.actor, 0.5, top ? St.Side.TOP : St.Side.BOTTOM);
        for (const item of this._items) {
            if (item.separator) {
                menu.addMenuItem(new PopupMenu.PopupSeparatorMenuItem());
                continue;
            }
            const inert = !item.action;
            const entry = new PopupMenu.PopupMenuItem(item.label, inert ? {reactive: false, can_focus: false} : {});
            entry.label.clutter_text.use_markup = false;
            if (inert)
                entry.remove_style_class_name('popup-inactive-menu-item');
            else
                entry.connect('activate', () => this._module.act(this.plugin.id, item.action));
            menu.addMenuItem(entry);
        }
        Main.uiGroup.add_child(menu.actor);
        menu.actor.hide();
        this._manager = new PopupMenu.PopupMenuManager(this.actor);
        this._manager.addMenu(menu);
        this._menu = menu;
        menu.open();
    }

    _closeMenu() {
        const menu = this._menu;
        this._menu = null;
        if (menu) {
            this._manager?.removeMenu(menu);
            menu.destroy();
        }
        this._manager = null;
    }

    destroy() {
        this._closeMenu();
        this.actor.destroy();
    }
}

export class DeskWidgets {
    constructor(ext) {
        this._ext = ext;
    }

    enable() {
        this._settings = this._ext.getSettings(SCHEMA);
        this._settings.connectObject('changed::enabled', () => this._sync(), this);
        this._sync();
    }

    disable() {
        this._stop();
        this._settings?.disconnectObject(this);
        // Disposed now rather than by the collector (dock.js says why)
        this._settings?.run_dispose();
        this._settings = null;
    }

    get on() {
        return this._settings?.get_boolean('enabled') ?? false;
    }

    get _plugins() {
        return this._ext.modules?.get('plugins') ?? null;
    }

    get palette() {
        return this._plugins?.palette ?? {};
    }

    pausedWords(plugin) {
        return this._plugins?.pausedWords(plugin) ?? '';
    }

    act(id, action) {
        this._plugins?.act(id, action);
    }

    _sync() {
        if (this.on)
            this._start();
        else
            this._stop();
    }

    // The plugins module starts before this one and tells it whenever what plugins show changes
    _start() {
        if (this._layer)
            return;
        this._cards = new Map();
        this._layer = new St.Widget({name: 'amethystoraDeskWidgets', reactive: false});
        global.window_group.add_child(this._layer);
        this._place();
        // After the restack and after the pictures under the windows have gone back down
        global.display.connectObject('restacked', () => {
            this._placeLater ||= global.compositor.get_laters().add(Meta.LaterType.BEFORE_REDRAW, () => {
                this._placeLater = 0;
                this._place();
                return false;
            });
        }, this);
        global.display.connectObject('workareas-changed', () => this.queueLayout(), this);
        Main.layoutManager.connectObject('monitors-changed', () => this.queueLayout(), this);
        this._attach();
    }

    _attach() {
        this._unlisten?.();
        this._unlisten = this._plugins?.onChanged(() => this._refresh()) ?? null;
        this._refresh();
    }

    _refresh() {
        if (!this._layer)
            return;
        const wanted = new Map(this._plugins?.desktopPlugins().map(p => [p.id, p]) ?? []);
        for (const [id, card] of [...this._cards]) {
            if (!wanted.has(id)) {
                this._unload(id);
                card.destroy();
            }
        }
        for (const [id, plugin] of wanted) {
            const card = this._cards.get(id);
            if (card) {
                card.set(plugin);
                continue;
            }
            const made = new Card(this, plugin);
            this._cards.set(id, made);
            this._layer.add_child(made.actor);
            made.actor.connectObject('notify::width', () => this.queueLayout(), 'notify::height', () => this.queueLayout(), this);
            this.queueLayout();
            this._ext.modules?.get('glass')?.surfacesChanged?.();
        }
    }

    _stop() {
        this._unlisten?.();
        this._unlisten = null;
        if (this._layoutLater)
            global.compositor.get_laters().remove(this._layoutLater);
        this._layoutLater = 0;
        if (this._placeLater)
            global.compositor.get_laters().remove(this._placeLater);
        this._placeLater = 0;
        global.display.disconnectObject(this);
        Main.layoutManager.disconnectObject(this);
        this._cards?.forEach(card => card.destroy());
        const hadLayer = Boolean(this._layer);
        this._layer?.destroy();
        this._layer = null;
        this._cards = null;
        if (hadLayer)
            this._ext.modules?.get('glass')?.surfacesChanged?.();
    }

    // Right under the first window, over every layer of pictures under it
    _place() {
        const group = global.window_group;
        const first = group.get_children().find(child => child instanceof Meta.WindowActor && child !== this._layer);
        if (first)
            group.set_child_below_sibling(this._layer, first);
        else
            group.set_child_above_sibling(this._layer, null);
    }

    _unload(id) {
        const card = this._cards?.get(id);
        if (!card)
            return;
        this._cards.delete(id);
        card.actor.disconnectObject(this);
        this._ext.modules?.get('glass')?.surfacesChanged?.();
    }

    queueLayout() {
        if (this._layoutLater || !this._layer)
            return;
        this._layoutLater = global.compositor.get_laters().add(Meta.LaterType.BEFORE_REDRAW, () => {
            this._layoutLater = 0;
            this._layout();
            return false;
        });
    }

    // Each corner of the primary screen's work area, its cards stacked from the corner inward, in "order" and then by
    // folder name
    _layout() {
        if (!this._layer)
            return;
        const area = Main.layoutManager.getWorkAreaForMonitor(Main.layoutManager.primaryIndex);
        const byCorner = new Map();
        for (const [, card] of [...this._cards].sort(([a, x], [b, y]) => x.rank - y.rank || a.localeCompare(b))) {
            if (!card.actor.visible)
                continue;
            const list = byCorner.get(card.corner) ?? [];
            list.push(card);
            byCorner.set(card.corner, list);
        }
        const rtl = Clutter.get_default_text_direction() === Clutter.TextDirection.RTL;
        for (const [corner, cards] of byCorner) {
            let [vertical, horizontal] = corner.split('-');
            // In a right-to-left language the corners are mirrored, as the rest of the desktop is
            if (rtl)
                horizontal = horizontal === 'left' ? 'right' : 'left';
            let offset = EDGE;
            for (const card of cards) {
                const [, width] = card.actor.get_preferred_width(-1);
                const [, height] = card.actor.get_preferred_height(width);
                const x = horizontal === 'left' ? area.x + EDGE : area.x + area.width - EDGE - width;
                const y = vertical === 'top' ? area.y + offset : area.y + area.height - offset - height;
                card.actor.set_position(Math.round(x), Math.round(y));
                offset += height + GAP;
            }
        }
    }

    glassSurfaces() {
        return [...this._cards?.values() ?? []].map(card => ({actor: card.actor, material: 'glass', radius: null}));
    }

    // For the VM suite: the cards as a person sees them
    cards() {
        return [...this._cards?.entries() ?? []].map(([id, card]) => ({
            id, corner: card.corner, visible: card.actor.visible, x: card.actor.x, y: card.actor.y,
            width: card.actor.width, height: card.actor.height, error: card.plugin.error || null,
            text: card.output?.text ?? null,
        }));
    }
}
