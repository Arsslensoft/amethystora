// Widgets on the desktop (board 49, R4-19; offered, off). A widget of ~/.config/amethystora/widgets whose widget.json
// says "place": "desktop" is a small glass card on the wallpaper of the primary screen, under every window, in the
// corner its "position" names. It is the same widget as the top bar's: the same format, read by the widgets extension's
// protocol.js, and the same runner (runner.js: its folder, a fixed PATH, 30 seconds a run, a session of its own), both
// imported from that extension's folder beside this one; nothing new to learn and nothing new that runs. The bar leaves
// such a widget alone, and while this is off it is drawn nowhere and its command never runs.
//
// The cards are a layer of the window group right under the first window: over the wallpaper, a workspace's own
// picture and Focus Sleep's night picture, which sit in the same group, and put back there whenever Mutter restacks
// the windows (a restack lowers only the windows and the background group). They run only while the widgets extension
// itself is on, from its own folder. Off, only the settings are connected.

import Clutter from 'gi://Clutter';
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import Meta from 'gi://Meta';
import Pango from 'gi://Pango';
import St from 'gi://St';

import {gettext as _} from 'resource:///org/gnome/shell/extensions/extension.js';
import {ExtensionState} from 'resource:///org/gnome/shell/misc/extensionUtils.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import * as PopupMenu from 'resource:///org/gnome/shell/ui/popupMenu.js';

const SCHEMA = 'org.gnome.shell.extensions.amethystora-ui.deskwidgets';
const WIDGETS_UUID = 'amethystora-widgets@arsslensoft.github.io';
// The theme amethystora-theme applies, whose palette colours a widget's class, as the bar's
const THEME_DIR = GLib.build_filenamev([GLib.get_user_config_dir(), 'amethystora', 'current']);
// The account turned the AI agent on (ame agent toggle on): the agent is offered only then
const AGENTIC = GLib.build_filenamev([GLib.get_user_config_dir(), 'amethystora', 'agentic']);
const CLASS_COLORS = {accent: 'accent', good: 'color2', warning: 'color3', critical: 'color1'};
// From the work area's edges, and between two cards in a corner
const EDGE = 16;
const GAP = 12;
// Milliseconds for a burst of saves to settle before anything is loaded again
const SETTLE = 300;
// A card shows at most this many lines of a widget's text, the last cut short
const MOST_LINES = 6;
const ROOT = Symbol('root');
const THEME = Symbol('theme');

function readPalette() {
    try {
        const [, bytes] = GLib.file_get_contents(GLib.build_filenamev([THEME_DIR, 'theme', 'colors.toml']));
        const text = new TextDecoder().decode(bytes);
        return Object.fromEntries([...text.matchAll(/^(\w+)\s*=\s*"(#[0-9a-fA-F]{3,8})"/gm)].map(m => [m[1], m[2]]));
    } catch {
        return {};
    }
}

function watch(path, callback) {
    try {
        const monitor = Gio.File.new_for_path(path).monitor_directory(Gio.FileMonitorFlags.WATCH_MOVES, null);
        monitor.connect('changed', callback);
        return monitor;
    } catch (e) {
        console.warn(`Amethystora UI: ${path} cannot be watched: ${e.message}`);
        return null;
    }
}

function textLabel(text, styleClass) {
    const label = new St.Label({text, style_class: styleClass, y_align: Clutter.ActorAlign.CENTER});
    label.clutter_text.use_markup = false;
    label.clutter_text.ellipsize = Pango.EllipsizeMode.END;
    return label;
}

// A path relative to the widget's folder, or to the home folder with ~/
function resolve(widget, path) {
    path = path.replace(/^~(?=\/|$)/, GLib.get_home_dir());
    return GLib.path_is_absolute(path) ? path : GLib.build_filenamev([widget.dir, path]);
}

function gicon(widget, name) {
    return name.includes('/')
        ? new Gio.FileIcon({file: Gio.File.new_for_path(resolve(widget, name))})
        : new Gio.ThemedIcon({name});
}

class Card {
    constructor(module, lib, widget) {
        this._module = module;
        this._lib = lib;
        this.widget = widget;
        this.state = null;
        this.error = null;
        this._menuKey = null;
        this._items = [];

        this.actor = new St.BoxLayout({
            style_class: 'amethystora-deskwidgets-card',
            orientation: Clutter.Orientation.VERTICAL,
            reactive: true,
            track_hover: true,
            can_focus: true,
            accessible_name: widget.manifest?.name ?? widget.id,
            visible: false,
        });
        const click = new Clutter.ClickGesture();
        click.connect('recognize', () => this._clicked());
        this.actor.add_action(click);

        if (widget.errors.length) {
            this._fail(widget.errors.join('\n'));
        } else {
            this._runner = new lib.Runner(widget, state => this._show(state), message => this._fail(message));
            this._runner.start();
        }
    }

    get corner() {
        return this.widget.manifest?.position ?? 'top-right';
    }

    get rank() {
        return this.widget.manifest?.order ?? 0;
    }

    // The card's lines: the icon beside the first, the rest smaller; the class colours the last line
    _draw(icon, lines, colour, warning = false) {
        this.actor.destroy_all_children();
        const large = this.widget.manifest?.size === 'large' && !warning;
        lines.forEach((line, i) => {
            const label = textLabel(line, i === 0
                ? `amethystora-deskwidgets-title${large ? ' amethystora-deskwidgets-large' : ''}`
                : 'amethystora-deskwidgets-line');
            if (colour && i === lines.length - 1)
                label.style = `color: ${colour};`;
            if (i === 0 && icon) {
                const row = new St.BoxLayout({style_class: 'amethystora-deskwidgets-head'});
                row.add_child(new St.Icon({gicon: icon, icon_size: 16, y_align: Clutter.ActorAlign.CENTER}));
                row.add_child(label);
                this.actor.add_child(row);
            } else {
                this.actor.add_child(label);
            }
        });
        if (warning)
            this.actor.add_style_class_name('amethystora-deskwidgets-broken');
        else
            this.actor.remove_style_class_name('amethystora-deskwidgets-broken');
    }

    _show(state) {
        this.state = state;
        this.error = null;
        this.paint();
        this._fillMenu(state.menu);
        this.actor.visible = !state.hidden;
        if (state.hidden)
            this._menu?.close();
        this._module.queueLayout();
    }

    paint() {
        if (this.error) {
            const colour = this._module.palette.color3;
            this._draw(new Gio.ThemedIcon({name: 'dialog-warning-symbolic'}),
                [this.widget.manifest?.name ?? this.widget.id], colour, true);
            return;
        }
        const state = this.state;
        if (!state)
            return;
        const lines = state.text.split('\n').filter(line => line.trim() !== '').slice(0, MOST_LINES);
        if (lines.length === 0)
            lines.push('');
        this._draw(state.icon ? gicon(this.widget, state.icon) : null, lines,
            this._module.palette[CLASS_COLORS[state.class]] ?? null);
    }

    // What went wrong, in its menu, with the agent offered to fix it while it is on
    _fail(message) {
        this.state = null;
        this.error = message;
        this.actor.visible = true;
        this.paint();
        this._fillMenu([
            {label: `${this.widget.manifest?.name ?? this.widget.id}: ${message}`},
            {separator: true},
            ...GLib.file_test(AGENTIC, GLib.FileTest.EXISTS)
                ? [{label: _('Fix it with the AI agent'), run: ['amethystora-widgets', 'fix', this.widget.id]}] : [],
            {label: _('Open its folder'), open: this.widget.dir},
        ]);
        this._module.queueLayout();
    }

    _clicked() {
        if (this.state?.click && this._items.length === 0) {
            this._act(this.state.click);
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

    // The menu is made when it opens, from the last update's items, as the bar makes its own
    _fillMenu(items) {
        const key = JSON.stringify(items);
        if (key === this._menuKey)
            return;
        this._menuKey = key;
        this._items = items;
        if (this._menu?.isOpen)
            this._openMenu();
    }

    _openMenu() {
        this._closeMenu();
        const top = this.corner.startsWith('top');
        const menu = new PopupMenu.PopupMenu(this.actor, 0.5, top ? St.Side.TOP : St.Side.BOTTOM);
        for (const item of this._items)
            menu.addMenuItem(this._menuItem(item));
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

    _menuItem(item) {
        if (item.separator === true)
            return new PopupMenu.PopupSeparatorMenuItem();
        if (typeof item.toggle === 'boolean') {
            const toggle = new PopupMenu.PopupSwitchMenuItem(item.label, item.toggle);
            toggle.label.clutter_text.use_markup = false;
            toggle.connect('toggled', () => this._act(item.run));
            return toggle;
        }
        // A line with nothing to do is information, neither greyed out nor highlighted
        const inert = !item.run && !item.open;
        const params = inert ? {reactive: false, can_focus: false} : {};
        const menuItem = item.icon
            ? new PopupMenu.PopupImageMenuItem(item.label, gicon(this.widget, item.icon), params)
            : new PopupMenu.PopupMenuItem(item.label, params);
        menuItem.label.clutter_text.use_markup = false;
        if (inert) {
            menuItem.remove_style_class_name('popup-inactive-menu-item');
            menuItem.label.add_style_class_name('amethystora-deskwidgets-menu-line');
            menuItem.label.clutter_text.ellipsize = Pango.EllipsizeMode.NONE;
            menuItem.label.clutter_text.line_wrap = true;
        } else {
            menuItem.connect('activate', () => (item.run ? this._act(item.run) : this._open(item.open)));
        }
        return menuItem;
    }

    _act(command) {
        this._lib.runAction(this.widget, command, reason => {
            if (reason) {
                Main.notifyError(this.widget.manifest?.name ?? this.widget.id,
                    `${JSON.stringify(command)} ${reason}`);
            }
            this._runner?.refresh();
        });
    }

    _open(target) {
        const uri = GLib.uri_peek_scheme(target) ? target : Gio.File.new_for_path(resolve(this.widget, target)).get_uri();
        try {
            Gio.AppInfo.launch_default_for_uri(uri, global.create_app_launch_context(0, -1));
        } catch (e) {
            Main.notifyError(this.widget.manifest?.name ?? this.widget.id, e.message);
        }
    }

    destroy() {
        this._closeMenu();
        this._runner?.stop();
        this._runner = null;
        this.actor.destroy();
    }
}

export class DeskWidgets {
    constructor(ext) {
        this._ext = ext;
    }

    enable() {
        this._generation = 0;
        this._settings = this._ext.getSettings(SCHEMA);
        this._settings.connectObject('changed::enabled', () => this._sync(), this);
        this._sync();
    }

    disable() {
        Main.extensionManager.disconnectObject(this);
        this._stop();
        this._settings?.disconnectObject(this);
        // Disposed now rather than by the collector (dock.js says why)
        this._settings?.run_dispose();
        this._settings = null;
    }

    get on() {
        return this._settings?.get_boolean('enabled') ?? false;
    }

    // On, while the widgets extension is on: its state is followed only while this is
    _sync() {
        if (this.on) {
            Main.extensionManager.connectObject('extension-state-changed', (m, extension) => {
                if (extension.uuid === WIDGETS_UUID)
                    this._apply();
            }, this);
        } else {
            Main.extensionManager.disconnectObject(this);
        }
        this._apply();
    }

    get _widgets() {
        const extension = Main.extensionManager.lookup(WIDGETS_UUID);
        return extension?.state === ExtensionState.ACTIVE ? extension : null;
    }

    _apply() {
        if (this.on && this._widgets) {
            this._start().catch(e => {
                console.error(`Amethystora UI: widgets on the desktop did not start: ${e.message}\n${e.stack}`);
                this._stop();
            });
        } else {
            this._stop();
        }
    }

    // The widgets extension's own runner and format, from its folder beside this extension's: a missing or broken
    // widgets extension costs this module alone
    async _start() {
        if (this._layer || this._loading)
            return;
        const generation = ++this._generation;
        this._loading = true;
        let lib;
        try {
            // The widgets extension's own folder, the account's copy where it has one
            lib = await import(this._widgets.dir.get_child('runner.js').get_uri());
        } catch (e) {
            console.error(`Amethystora UI: widgets on the desktop need ${WIDGETS_UUID}: ${e.message}`);
            return;
        } finally {
            this._loading = false;
        }
        // Turned off and on again while the runner was being read: started again now
        if (generation !== this._generation || !this._settings) {
            if (this._settings)
                this._apply();
            return;
        }
        if (!this.on || !this._widgets)
            return;
        this._lib = lib;
        this.palette = readPalette();
        this._cards = new Map();
        this._folders = new Map();
        this._pending = new Set();
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
        GLib.mkdir_with_parents(lib.WIDGETS_DIR, 0o755);
        this._root = watch(lib.WIDGETS_DIR, () => this._queue(ROOT));
        this._theme = watch(THEME_DIR, () => this._queue(THEME));
        global.display.connectObject('workareas-changed', () => this.queueLayout(), this);
        Main.layoutManager.connectObject('monitors-changed', () => this.queueLayout(), this);
        this._scan();
    }

    _stop() {
        this._generation++;
        if (this._settle)
            GLib.source_remove(this._settle);
        this._settle = 0;
        if (this._layoutLater)
            global.compositor.get_laters().remove(this._layoutLater);
        this._layoutLater = 0;
        if (this._placeLater)
            global.compositor.get_laters().remove(this._placeLater);
        this._placeLater = 0;
        global.display.disconnectObject(this);
        Main.layoutManager.disconnectObject(this);
        this._root?.cancel();
        this._theme?.cancel();
        this._folders?.forEach(monitor => monitor?.cancel());
        this._cards?.forEach(card => card.destroy());
        const hadLayer = Boolean(this._layer);
        this._layer?.destroy();
        this._layer = null;
        this._root = this._theme = this._folders = this._cards = this._pending = null;
        this._lib = null;
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

    _queue(what) {
        this._pending?.add(what);
        if (this._settle || !this._pending)
            return;
        this._settle = GLib.timeout_add(GLib.PRIORITY_DEFAULT, SETTLE, () => {
            this._settle = 0;
            const pending = [...this._pending ?? []];
            this._pending?.clear();
            for (const id of pending) {
                if (id === THEME) {
                    this.palette = readPalette();
                    this._cards?.forEach(card => card.paint());
                } else if (id === ROOT) {
                    this._scan();
                } else {
                    this._load(id);
                }
            }
            return GLib.SOURCE_REMOVE;
        });
    }

    // Every folder is watched, also those of the bar, so that a change of place is noticed
    _scan() {
        const ids = [];
        try {
            const children = Gio.File.new_for_path(this._lib.WIDGETS_DIR).enumerate_children(
                'standard::name,standard::type', Gio.FileQueryInfoFlags.NONE, null);
            let info;
            while ((info = children.next_file(null))) {
                if (info.get_file_type() === Gio.FileType.DIRECTORY && !info.get_name().startsWith('.'))
                    ids.push(info.get_name());
            }
        } catch (e) {
            console.warn(`Amethystora UI: the widgets cannot be read: ${e.message}`);
        }
        for (const id of ids) {
            if (this._folders.has(id))
                continue;
            this._folders.set(id, watch(GLib.build_filenamev([this._lib.WIDGETS_DIR, id]), () => this._queue(id)));
            this._load(id);
        }
        for (const [id, monitor] of this._folders) {
            if (ids.includes(id))
                continue;
            monitor?.cancel();
            this._folders.delete(id);
            this._unload(id);
        }
    }

    _load(id) {
        this._unload(id);
        const dir = GLib.build_filenamev([this._lib.WIDGETS_DIR, id]);
        // amethystora-widgets off leaves this file in the folder
        if (!GLib.file_test(dir, GLib.FileTest.IS_DIR) ||
            GLib.file_test(GLib.build_filenamev([dir, 'disabled']), GLib.FileTest.EXISTS))
            return;
        const widget = this._lib.loadWidget(dir);
        // The desktop's widgets only; a widget.json that cannot be read stays the bar's warning sign
        if (widget.manifest?.place !== 'desktop' && !this._saysDesktop(dir))
            return;
        try {
            const card = new Card(this, this._lib, widget);
            this._cards.set(id, card);
            this._layer.add_child(card.actor);
            card.actor.connectObject('notify::width', () => this.queueLayout(),
                'notify::height', () => this.queueLayout(), this);
            this.queueLayout();
            this._ext.modules?.get('glass')?.surfacesChanged?.();
        } catch (e) {
            console.error(`Amethystora UI: the widget ${id} could not be drawn on the desktop: ${e.message}`);
            this._unload(id);
        }
    }

    // A desktop widget whose widget.json has a mistake in it is still the desktop's: its card says so
    _saysDesktop(dir) {
        try {
            const [, bytes] = GLib.file_get_contents(GLib.build_filenamev([dir, 'widget.json']));
            return JSON.parse(new TextDecoder().decode(bytes))?.place === 'desktop';
        } catch {
            return false;
        }
    }

    _unload(id) {
        const card = this._cards?.get(id);
        if (!card)
            return;
        this._cards.delete(id);
        card.actor.disconnectObject(this);
        card.destroy();
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
            width: card.actor.width, height: card.actor.height, error: card.error,
            text: card.state?.text ?? null,
        }));
    }
}
