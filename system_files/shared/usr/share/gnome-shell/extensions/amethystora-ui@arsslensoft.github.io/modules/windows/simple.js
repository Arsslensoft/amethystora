// A desktop for your parents (R7-65; offered, off, per account): big tiles on the wallpaper for the few apps a person
// uses, large text, a larger pointer, one window at a time, a plain top bar with a Home button, and a press of Super that
// opens nothing. Nothing is taken out of the system: these are keys that exist, set together, and turning it off
// (Super+Alt+Home, `ame desktop windows simple-desktop off`, Control) puts back what each key held.
//
// What each key held before is in ~/.local/state/amethystora/simple.json, 0600: settings' values and nothing else, read
// and written asynchronously, and only when the switch changes or the part starts. A key the person changed meanwhile
// is left as they set it. A window keeps its size when this is turned off. Off, nothing here runs.

import Clutter from 'gi://Clutter';
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import Meta from 'gi://Meta';
import Shell from 'gi://Shell';
import St from 'gi://St';

import {gettext as _} from 'resource:///org/gnome/shell/extensions/extension.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import * as PanelMenu from 'resource:///org/gnome/shell/ui/panelMenu.js';

Gio._promisify(Gio.File.prototype, 'load_contents_async', 'load_contents_finish');
Gio._promisify(Gio.File.prototype, 'replace_contents_bytes_async', 'replace_contents_finish');
Gio._promisify(Gio.File.prototype, 'delete_async', 'delete_finish');
Gio._promisify(Gio.File.prototype, 'make_directory_async', 'make_directory_finish');

const UI = 'org.gnome.shell.extensions.amethystora-ui';
const STATE = GLib.build_filenamev([GLib.get_user_state_dir(), 'amethystora', 'simple.json']);
const MOST_TILES = 6;
const ICON = 72;
const COLUMNS = 2;
const ROLE = 'amethystora-simple-home';
// What the bar keeps out of sight: the clock, the system menu, the logo menu and what a person needs stay
const BAR_HIDDEN = ['workspaces', 'tiling', 'busy', 'processor', 'memory', 'network'];

// [schema, key, what it is set to]: each has to exist, which 20-tests.sh holds
export const SETS = [
    ['org.gnome.desktop.interface', 'text-scaling-factor', () => new GLib.Variant('d', 1.25)],
    ['org.gnome.desktop.interface', 'cursor-size', () => new GLib.Variant('i', 36)],
    [`${UI}.tiling`, 'enabled', () => new GLib.Variant('b', false)],
    [`${UI}.keys`, 'super-alone', () => new GLib.Variant('s', 'nothing')],
    [`${UI}.dock`, 'icon-size', () => new GLib.Variant('i', 64)],
    [`${UI}.bars`, 'layout', () => new GLib.Variant('a{sas}', {hidden: BAR_HIDDEN})],
];

async function readState() {
    try {
        const [contents] = await Gio.File.new_for_path(STATE).load_contents_async(null);
        const state = JSON.parse(new TextDecoder().decode(contents));
        return state && typeof state === 'object' && !Array.isArray(state) ? state : {};
    } catch {
        return null;
    }
}

// The folder and the ones above it, without waiting for the disk
async function makeFolders(folder) {
    try {
        await folder.make_directory_async(GLib.PRIORITY_DEFAULT, null);
    } catch (e) {
        if (e.matches(Gio.IOErrorEnum, Gio.IOErrorEnum.EXISTS))
            return;
        if (!e.matches(Gio.IOErrorEnum, Gio.IOErrorEnum.NOT_FOUND))
            throw e;
        await makeFolders(folder.get_parent());
        await makeFolders(folder);
    }
}

// Applying and restoring read and write the same file: one after the other, so that a switch turned on and off in quick
// succession never reads a file that is only half-way written, nor leaves one behind
let queue = Promise.resolve();
const inTurn = work => {
    queue = queue.then(work, work);
    return queue;
};

export class Simple {
    constructor(module) {
        this._module = module;
    }

    enable() {
        this._alive = {};
        const settings = this._module.settings;
        this._build();
        Shell.AppSystem.get_default().connectObject('installed-changed', () => this._fill(), this);
        Main.layoutManager.connectObject('monitors-changed', () => this._place(), this);
        global.display.connectObject(
            'restacked', () => this._lower(),
            'window-created', (display, window) => this._follow(window), this);
        settings.connectObject('changed::simple-tiles', () => this._fill(), this);
        Main.wm.addKeybinding('simple-desktop-leave', settings, Meta.KeyBindingFlags.IGNORE_AUTOREPEAT,
            Shell.ActionMode.NORMAL | Shell.ActionMode.OVERVIEW, () => settings.set_boolean('simple-desktop', false));
        this._addHome();
        // A login opens on the overview, which is what this leaves out
        if (Main.layoutManager._startingUp)
            Main.layoutManager.connectObject('startup-complete', () => Main.overview.hide(), this);
        this._applyOnce(this._alive).catch(logError);
    }

    disable() {
        // What the deferred work above looks at
        this._alive = null;
        Main.wm.removeKeybinding('simple-desktop-leave');
        this._module.settings?.disconnectObject(this);
        global.display.disconnectObject(this);
        Main.layoutManager.disconnectObject(this);
        Shell.AppSystem.get_default().disconnectObject(this);
        for (const window of this._waiting ?? [])
            window.disconnectObject(this);
        this._waiting = null;
        this._home?.destroy();
        this._home = null;
        this._layer?.destroy();
        this._layer = null;
        this._tiles = [];
        this._module.surfacesChanged();
    }

    // ---- The keys set together ----

    // Once per turning on: a file there means it has been done, a lock and unlock or a restart ago
    _applyOnce(alive) {
        return inTurn(() => this._apply(alive));
    }

    async _apply(alive) {
        if (alive !== this._alive || await readState() !== null || alive !== this._alive)
            return;
        const old = {};
        const each = [];
        for (const [schema, key, value] of SETS) {
            const settings = this._module.ext.getSettings(schema);
            old[`${schema} ${key}`] = settings.get_user_value(key)?.print(true) ?? null;
            each.push([settings, key, value()]);
        }
        // What each key held is kept before any is set, or none is set: a key that cannot be given back is never taken
        let kept = true;
        try {
            await makeFolders(Gio.File.new_for_path(GLib.path_get_dirname(STATE)));
            await Gio.File.new_for_path(STATE).replace_contents_bytes_async(
                new GLib.Bytes(new TextEncoder().encode(JSON.stringify(old))), null, false,
                Gio.FileCreateFlags.PRIVATE | Gio.FileCreateFlags.REPLACE_DESTINATION, null);
        } catch (e) {
            kept = false;
            console.warn(`Amethystora UI: the simple desktop could not keep the settings it replaces: ${e.message}`);
        }
        for (const [settings, key, value] of each) {
            if (kept && alive === this._alive)
                settings.set_value(key, value);
            settings.run_dispose();
        }
        if (alive !== this._alive)
            return;
        if (!kept) {
            // Turned off again, and said: tiles with none of the settings would be a half of it
            this._module.settings.set_boolean('simple-desktop', false);
            Main.notify(_('A simple desktop'), _('It could not be turned on: there was nowhere to keep your settings, so none was changed.'));
            return;
        }
        // Once, as it is turned on: a person who made a window smaller keeps it so through the next lock
        for (const actor of global.get_window_actors())
            this._maximize(actor.meta_window);
        Main.notify(_('A simple desktop'),
            _('Big tiles, large text and one window at a time. Super+Alt+Home goes back to the full desktop.'));
    }

    // The switch is off (turned off by the person, windows.js, once this part has stopped, or found off at the start,
    // when it was turned off while the shell was not running): what each key held comes back
    restore() {
        const settings = this._module.settings;
        return inTurn(() => this._restore(settings));
    }

    async _restore(alive) {
        const state = await readState();
        if (state === null || this._module.settings !== alive)
            return;
        for (const [schema, key, value] of SETS) {
            const name = `${schema} ${key}`;
            if (!(name in state))
                continue;
            const settings = this._module.ext.getSettings(schema);
            try {
                // Changed since by the person: theirs
                if (settings.get_value(key).equal(value())) {
                    if (state[name] === null)
                        settings.reset(key);
                    else
                        settings.set_value(key, GLib.Variant.parse(null, state[name], null, null));
                }
            } catch (e) {
                console.warn(`Amethystora UI: ${key} could not be put back: ${e.message}`);
            }
            settings.run_dispose();
        }
        try {
            await Gio.File.new_for_path(STATE).delete_async(GLib.PRIORITY_DEFAULT, null);
        } catch {}
    }

    // ---- One window at a time ----

    _maximize(window) {
        if (window.get_window_type() === Meta.WindowType.NORMAL && !window.get_transient_for() && window.can_maximize())
            window.maximize();
    }

    // A window can be maximized once it is shown
    _follow(window) {
        this._waiting ??= new Set();
        this._waiting.add(window);
        window.connectObject(
            'shown', () => {
                window.disconnectObject(this);
                this._waiting?.delete(window);
                this._maximize(window);
            },
            'unmanaged', () => {
                window.disconnectObject(this);
                this._waiting?.delete(window);
            }, this);
    }

    // Home: the windows of this workspace go down, and the tiles are there
    _goHome() {
        const workspace = global.workspace_manager.get_active_workspace();
        for (const window of global.display.list_all_windows()) {
            if (window.located_on_workspace(workspace) && window.can_minimize() && !window.is_skip_taskbar())
                window.minimize();
        }
        Main.overview.hide();
    }

    _addHome() {
        const home = new PanelMenu.Button(0.5, _('Home'), true);
        const box = new St.BoxLayout({style_class: 'amethystora-windows-simple-home'});
        box.add_child(new St.Icon({icon_name: 'go-home-symbolic', style_class: 'system-status-icon', y_align: Clutter.ActorAlign.CENTER}));
        box.add_child(new St.Label({text: _('Home'), y_align: Clutter.ActorAlign.CENTER}));
        home.add_child(box);
        const click = new Clutter.ClickGesture();
        click.connect('recognize', () => this._goHome());
        home.add_action(click);
        Main.panel.addToStatusArea(ROLE, home, 1, 'left');
        this._home = home;
    }

    // ---- The tiles ----

    // The first four by what they are for, in the words a person uses; or the ones chosen, by their launchers
    _wanted() {
        const system = Shell.AppSystem.get_default();
        const chosen = this._module.settings.get_strv('simple-tiles');
        if (chosen.length) {
            return chosen.slice(0, MOST_TILES).map(id => system.lookup_app(id)).filter(app => app)
                .map(app => ({app, name: app.get_name()}));
        }
        return [
            ['x-scheme-handler/https', _('The internet')],
            ['x-scheme-handler/mailto', _('Email')],
            ['image/jpeg', _('Photos')],
            ['inode/directory', _('Files')],
        ].map(([type, name]) => {
            const id = Gio.AppInfo.get_default_for_type(type, false)?.get_id();
            return {app: id ? system.lookup_app(id) : null, name};
        }).filter(tile => tile.app);
    }

    _build() {
        this._layer = new St.Widget({name: 'amethystoraSimpleTiles', reactive: false});
        global.window_group.add_child(this._layer);
        this._grid = new St.BoxLayout({
            style_class: 'amethystora-windows-simple-grid',
            orientation: Clutter.Orientation.VERTICAL,
        });
        this._layer.add_child(this._grid);
        this._grid.connect('notify::width', () => this._place());
        this._grid.connect('notify::height', () => this._place());
        this._fill();
        this._lower();
    }

    _fill() {
        if (!this._grid)
            return;
        this._grid.destroy_all_children();
        this._tiles = [];
        let row = null;
        this._wanted().forEach(({app, name}, i) => {
            if (i % COLUMNS === 0) {
                row = new St.BoxLayout({style_class: 'amethystora-windows-simple-row'});
                this._grid.add_child(row);
            }
            const content = new St.BoxLayout({
                orientation: Clutter.Orientation.VERTICAL,
                x_align: Clutter.ActorAlign.CENTER,
                y_align: Clutter.ActorAlign.CENTER,
                style_class: 'amethystora-windows-simple-content',
            });
            const picture = app.create_icon_texture(ICON);
            picture.x_align = Clutter.ActorAlign.CENTER;
            content.add_child(picture);
            const label = new St.Label({text: name, x_align: Clutter.ActorAlign.CENTER, style_class: 'amethystora-windows-simple-name'});
            label.clutter_text.use_markup = false;
            content.add_child(label);
            const tile = new St.Button({
                style_class: 'amethystora-deskwidgets-card amethystora-windows-simple-tile',
                child: content,
                can_focus: true,
                accessible_name: name,
            });
            tile.connect('clicked', () => app.activate());
            row.add_child(tile);
            this._tiles.push(tile);
        });
        this._place();
        this._module.surfacesChanged();
    }

    _place() {
        if (!this._grid)
            return;
        const area = Main.layoutManager.getWorkAreaForMonitor(Main.layoutManager.primaryIndex);
        this._grid.set_position(Math.round(area.x + (area.width - this._grid.width) / 2),
            Math.round(area.y + (area.height - this._grid.height) / 2));
    }

    // Over the wallpaper and under every window, where a restack would otherwise lower it
    _lower() {
        const group = global.window_group;
        const first = group.get_children().find(child => child instanceof Meta.WindowActor);
        if (first)
            group.set_child_below_sibling(this._layer, first);
        else
            group.set_child_above_sibling(this._layer, null);
    }

    glassSurfaces() {
        return (this._tiles ?? []).map(actor => ({actor, material: 'glass', radius: null}));
    }
}
