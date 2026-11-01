// The top bar's widgets: each folder in ~/.config/amethystora/widgets is a widget, a command whose
// output the bar shows as a label, an icon and a menu (protocol.js says what it may print, runner.js
// runs it). A widget whose widget.json says "place": "desktop" is not the bar's: Amethystora UI's
// deskwidgets module draws it on the wallpaper, with this runner, while widgets on the desktop are on. A widget is loaded again as soon as anything in its folder changes, so one being written
// shows up as it is saved. The commands run outside GNOME Shell, which on Wayland is the compositor:
// a broken widget shows a warning sign in the bar, and cannot take the session down with it.

import Clutter from 'gi://Clutter';
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import GObject from 'gi://GObject';
import Pango from 'gi://Pango';
import St from 'gi://St';

import {Extension, gettext as _} from 'resource:///org/gnome/shell/extensions/extension.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import * as PanelMenu from 'resource:///org/gnome/shell/ui/panelMenu.js';
import * as PopupMenu from 'resource:///org/gnome/shell/ui/popupMenu.js';

import {Runner, WIDGETS_DIR, loadWidget, runAction} from './runner.js';

// amethystora-theme swaps the current theme into this folder; its palette gives each class its colour
const THEME_DIR = GLib.build_filenamev([GLib.get_user_config_dir(), 'amethystora', 'current']);
// The account turned the AI agent on (ame agent toggle on): the agent is offered only then
const AGENTIC = GLib.build_filenamev([GLib.get_user_config_dir(), 'amethystora', 'agentic']);
const CLASS_COLORS = {accent: 'accent', good: 'color2', warning: 'color3', critical: 'color1'};
// Milliseconds for a burst of saves to settle before anything is loaded again
const SETTLE = 300;
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

// A folder that cannot be watched costs only the reload on save, never the widgets themselves
function watch(path, callback) {
    try {
        const monitor = Gio.File.new_for_path(path).monitor_directory(Gio.FileMonitorFlags.WATCH_MOVES, null);
        monitor.connect('changed', callback);
        return monitor;
    } catch (e) {
        console.warn(`Amethystora Widgets: ${path} cannot be watched: ${e.message}`);
        return null;
    }
}

// A path relative to the widget's folder, or to the home folder with ~/
function resolve(widget, path) {
    path = path.replace(/^~(?=\/|$)/, GLib.get_home_dir());
    return GLib.path_is_absolute(path) ? path : GLib.build_filenamev([widget.dir, path]);
}

// The name of an icon in the icon theme, or a picture
function icon(widget, name) {
    return name.includes('/')
        ? new Gio.FileIcon({file: Gio.File.new_for_path(resolve(widget, name))})
        : new Gio.ThemedIcon({name});
}

const WidgetButton = GObject.registerClass(
class WidgetButton extends PanelMenu.Button {
    _init(widget, host) {
        super._init(0.5, widget.manifest?.name ?? widget.id);
        this._widget = widget;
        this._host = host;
        this._state = null;
        this._error = null;
        this._menuKey = null;

        this._box = new St.BoxLayout({style_class: 'panel-status-menu-box'});
        this._icon = new St.Icon({style_class: 'system-status-icon', visible: false});
        this._label = new St.Label({
            style_class: 'amethystora-widget-label',
            y_align: Clutter.ActorAlign.CENTER,
            visible: false,
        });
        this._label.clutter_text.ellipsize = Pango.EllipsizeMode.END;
        this._box.add_child(this._icon);
        this._box.add_child(this._label);
        this.add_child(this._box);
        // Nothing to show until the command has printed something
        this.visible = false;

        this.connect('destroy', () => this._runner?.stop());
        if (widget.errors.length) {
            this._fail(widget.errors.join('\n'));
        } else {
            this._runner = new Runner(widget, state => this._show(state), message => this._fail(message));
            this._runner.start();
        }
    }

    get side() {
        return this._widget.manifest?.position ?? 'right';
    }

    get rank() {
        return this._widget.manifest?.order ?? 0;
    }

    _show(state) {
        this._state = state;
        this._error = null;
        if (state.hidden)
            this.menu.close();
        this.visible = !state.hidden;
        this._icon.visible = state.icon !== '';
        if (state.icon)
            this._icon.gicon = icon(this._widget, state.icon);
        this._label.visible = state.text !== '';
        this._label.text = state.text;
        this.paint();
        this._fillMenu(state.menu);
    }

    // What went wrong, in the menu of a warning sign, with the agent offered to fix it while it is on
    _fail(message) {
        this._state = null;
        this._error = message;
        this.visible = true;
        this._icon.visible = true;
        this._icon.icon_name = 'dialog-warning-symbolic';
        this._label.visible = false;
        this.paint();
        this._fillMenu([
            {label: `${this._widget.manifest?.name ?? this._widget.id}: ${message}`},
            {separator: true},
            ...GLib.file_test(AGENTIC, GLib.FileTest.EXISTS)
                ? [{label: _('Fix it with the AI agent'), run: ['amethystora-widgets', 'fix', this._widget.id]}] : [],
            {label: _('Open its folder'), open: this._widget.dir},
        ]);
    }

    paint() {
        const color = this._host.palette[this._error ? 'color3' : CLASS_COLORS[this._state?.class]];
        this._box.style = color ? `color: ${color};` : null;
    }

    // Built again only when it changed, so that an open menu is not rebuilt under the pointer on every update
    _fillMenu(items) {
        const key = JSON.stringify(items);
        if (key === this._menuKey)
            return;
        this._menuKey = key;
        this.menu.removeAll();
        for (const item of items)
            this.menu.addMenuItem(this._menuItem(item));
    }

    _menuItem(item) {
        if (item.separator === true)
            return new PopupMenu.PopupSeparatorMenuItem();
        if (typeof item.toggle === 'boolean') {
            const toggle = new PopupMenu.PopupSwitchMenuItem(item.label, item.toggle);
            toggle.connect('toggled', () => this._act(item.run));
            return toggle;
        }
        // A line with nothing to do is information, which is neither greyed out nor highlighted
        const inert = !item.run && !item.open;
        const params = inert ? {reactive: false, can_focus: false} : {};
        const menuItem = item.icon
            ? new PopupMenu.PopupImageMenuItem(item.label, icon(this._widget, item.icon), params)
            : new PopupMenu.PopupMenuItem(item.label, params);
        if (inert) {
            menuItem.remove_style_class_name('popup-inactive-menu-item');
            menuItem.label.add_style_class_name('amethystora-widget-line');
            menuItem.label.clutter_text.ellipsize = Pango.EllipsizeMode.NONE;
            menuItem.label.clutter_text.line_wrap = true;
        } else {
            menuItem.connect('activate', () => (item.run ? this._act(item.run) : this._open(item.open)));
        }
        return menuItem;
    }

    _act(command) {
        runAction(this._widget, command, reason => {
            if (reason)
                Main.notifyError(this._widget.manifest?.name ?? this._widget.id, `${JSON.stringify(command)} ${reason}`);
            this._runner?.refresh();
        });
    }

    _open(target) {
        const uri = GLib.uri_peek_scheme(target) ? target : Gio.File.new_for_path(resolve(this._widget, target)).get_uri();
        try {
            Gio.AppInfo.launch_default_for_uri(uri, global.create_app_launch_context(0, -1));
        } catch (e) {
            Main.notifyError(this._widget.manifest?.name ?? this._widget.id, e.message);
        }
    }

    // A widget with a "click" and no menu runs it, instead of opening a menu that would be empty
    vfunc_event(event) {
        const type = event.type();
        if (this._state?.click && this.menu.isEmpty() &&
            (type === Clutter.EventType.BUTTON_PRESS || type === Clutter.EventType.TOUCH_BEGIN)) {
            this._act(this._state.click);
            return Clutter.EVENT_STOP;
        }
        return super.vfunc_event(event);
    }
});

export default class WidgetsExtension extends Extension {
    enable() {
        this.palette = readPalette();
        this._buttons = new Map();
        this._folders = new Map();
        this._pending = new Set();
        this._settle = 0;
        GLib.mkdir_with_parents(WIDGETS_DIR, 0o755);
        this._root = watch(WIDGETS_DIR, () => this._queue(ROOT));
        this._theme = watch(THEME_DIR, () => this._queue(THEME));
        this._scan();
    }

    disable() {
        if (this._settle)
            GLib.source_remove(this._settle);
        this._root?.cancel();
        this._theme?.cancel();
        this._folders.forEach(monitor => monitor?.cancel());
        this._buttons.forEach(button => button.destroy());
        this._root = this._theme = this._folders = this._buttons = this._pending = null;
    }

    _queue(what) {
        this._pending.add(what);
        if (this._settle)
            return;
        this._settle = GLib.timeout_add(GLib.PRIORITY_DEFAULT, SETTLE, () => {
            this._settle = 0;
            const pending = [...this._pending];
            this._pending.clear();
            for (const id of pending) {
                if (id === THEME) {
                    this.palette = readPalette();
                    this._buttons.forEach(button => button.paint());
                } else if (id === ROOT) {
                    this._scan();
                } else {
                    this._load(id);
                }
            }
            return GLib.SOURCE_REMOVE;
        });
    }

    // Every folder is watched, also those turned off, so that turning one on again is noticed
    _scan() {
        const ids = [];
        try {
            const children = Gio.File.new_for_path(WIDGETS_DIR).enumerate_children(
                'standard::name,standard::type', Gio.FileQueryInfoFlags.NONE, null);
            let info;
            while ((info = children.next_file(null))) {
                if (info.get_file_type() === Gio.FileType.DIRECTORY && !info.get_name().startsWith('.'))
                    ids.push(info.get_name());
            }
        } catch (e) {
            console.warn(`Amethystora Widgets: ${WIDGETS_DIR} cannot be read: ${e.message}`);
        }
        for (const id of ids) {
            if (this._folders.has(id))
                continue;
            this._folders.set(id, watch(GLib.build_filenamev([WIDGETS_DIR, id]), () => this._queue(id)));
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
        const dir = GLib.build_filenamev([WIDGETS_DIR, id]);
        // amethystora-widgets off leaves this file in the folder
        if (!GLib.file_test(dir, GLib.FileTest.IS_DIR) ||
            GLib.file_test(GLib.build_filenamev([dir, 'disabled']), GLib.FileTest.EXISTS))
            return;
        try {
            const widget = loadWidget(dir);
            // Drawn on the wallpaper by Amethystora UI (its deskwidgets module), never in the bar; still watched, so
            // that a change of place is noticed
            if (widget.manifest?.place === 'desktop')
                return;
            const button = new WidgetButton(widget, this);
            this._buttons.set(id, button);
            Main.panel.addToStatusArea(`amethystora-widget-${id}`, button, 0, button.side);
            this._arrange();
        } catch (e) {
            console.error(`Amethystora Widgets: ${id} could not be loaded: ${e.message}`);
            this._unload(id);
        }
    }

    _unload(id) {
        this._buttons.get(id)?.destroy();
        this._buttons.delete(id);
    }

    // In "order", then by folder name, at the inner edge of their part of the bar: after what is on the
    // left and in the centre already, and before what is on the right, so the system menu stays last
    _arrange() {
        const buttons = [...this._buttons.entries()]
            .sort(([a, x], [b, y]) => x.rank - y.rank || a.localeCompare(b))
            .map(([, button]) => button);
        for (const button of buttons.filter(b => b.side !== 'right')) {
            const box = button.container.get_parent();
            box?.set_child_at_index(button.container, box.get_n_children() - 1);
        }
        for (const button of buttons.filter(b => b.side === 'right').reverse())
            button.container.get_parent()?.set_child_at_index(button.container, 0);
    }
}
