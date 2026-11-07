// What a workspace has of its own, from the settings: its wallpaper and colour (workspace-backgrounds,
// workspace-colors, default-background and use-default-background), its name (workspace-names) and the folder apps
// start in while it is shown (workspace-directories). Each screen draws the picture of the workspace it shows over
// GNOME's own wallpaper, with the workspace's colour under it where the picture leaves room. A workspace with no
// picture of its own, and no default one, shows GNOME's wallpaper as it is, so that the theme's wallpaper and its
// transitions are what one sees (or only its colour, when use-default-background is off). The pictures take no
// clicks: GNOME's background menu stays where it is.

import Clutter from 'gi://Clutter';
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import St from 'gi://St';

import {gettext as _} from 'resource:///org/gnome/shell/extensions/extension.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';

import {config} from './config.js';

// Under a picture that leaves room, a workspace without a colour of its own
const NEUTRAL = '#282828';

// The picture a workspace has, as a file, or null for GNOME's own
export function pictureOf(workspace) {
    const uri = config.workspaceBackgrounds[workspace.index()] || config.defaultBackground;
    if (!uri)
        return null;
    const file = uri.includes('://') ? Gio.File.new_for_uri(uri) : Gio.File.new_for_path(uri);
    return file.query_exists(null) ? file : null;
}

// The colour a workspace has, '#rrggbb', or null for the theme's
export function colourOf(workspace) {
    return config.workspaceColors[workspace.index()] || null;
}

// A workspace's name, the person's or "Workspace N"
export function nameOf(workspace) {
    const index = workspace.index();
    return config.workspaceNames[index]?.trim() || _('Workspace %d').replace('%d', String(index + 1));
}

// A screen's background with a workspace's picture over its colour, the picture filling the screen as GNOME's zoom
// does; its colour alone when it has no picture and GNOME's wallpaper is not wanted; or null for GNOME's wallpaper.
// Drawn by the shell's own stylesheet machinery, which loads the picture the way the shell loads every image, on
// GNOME 50 and 51 alike (Mutter's own background loading changed between them).
export function backgroundActor(workspace) {
    const file = pictureOf(workspace);
    const colour = `background-color: ${colourOf(workspace) ?? NEUTRAL};`;
    if (!file)
        return config.gnomeBackground ? null : new St.Widget({reactive: false, style: colour});
    const uri = file.get_uri().replace(/["\\]/g, c => `\\${c}`);
    return new St.Widget({
        reactive: false,
        // Covering, which St centres
        style: `${colour} background-image: url("${uri}"); background-size: cover;`,
    });
}

// The folder a workspace's apps start in, or null for none (or one that is not there)
function folderOf(workspace) {
    let folder = config.workspaceFolders[workspace.index()]?.trim() ?? '';
    if (folder === '~' || folder.startsWith('~/'))
        folder = GLib.get_home_dir() + folder.slice(1);
    return folder && GLib.file_test(folder, GLib.FileTest.IS_DIR) ? folder : null;
}

export class WorkspaceBackgrounds {
    constructor(engine) {
        this._engine = engine;
    }

    enable() {
        // Just above GNOME's backgrounds, under everything else in the window group (the pictures of the other
        // screens' workspaces included, which go above it)
        this.layer = new Clutter.Actor({reactive: false});
        const group = Main.layoutManager._backgroundGroup;
        if (group?.get_parent() === global.window_group)
            global.window_group.insert_child_above(this.layer, group);
        else
            global.window_group.insert_child_at_index(this.layer, 0);
        // A restack lowers the windows under every other actor of their group: the pictures go back under them
        global.display.connectObject('restacked', () => {
            if (group?.get_parent() === global.window_group && this.layer.get_previous_sibling() !== group)
                global.window_group.set_child_above_sibling(this.layer, group);
        }, this);
        this._shown = new Map();
        // The shell's own folder, which every app it starts begins in; changed only while a workspace has a folder
        this._startedIn = GLib.get_current_dir();
        this._folder = this._startedIn;
        global.workspace_manager.connectObject('active-workspace-changed', () => this.sync(), this);
        Main.layoutManager.connectObject('monitors-changed', () => this.sync(), this);
        // The tiling passes on the changes of the workspace- keys; this one is read here
        this._engine.tiling.settings?.connectObject('changed::use-default-background', () => this.sync(), this);
        this.sync();
    }

    disable() {
        global.display.disconnectObject(this);
        global.workspace_manager.disconnectObject(this);
        Main.layoutManager.disconnectObject(this);
        this._engine.tiling.settings?.disconnectObject(this);
        this.layer?.destroy();
        this.layer = null;
        this._shown = null;
        this._chdir(this._startedIn);
    }

    // Each screen with its workspace's picture, made again only where the workspace or its picture changed
    sync() {
        if (!this.layer)
            return;
        const monitors = Main.layoutManager.monitors;
        for (const [index, shown] of [...this._shown]) {
            if (index >= monitors.length) {
                shown.actor?.destroy();
                this._shown.delete(index);
            }
        }
        monitors.forEach((monitor, index) => {
            const workspace = this._engine.screens?.shownOn(index) ?? global.workspace_manager.get_active_workspace();
            const key = `${workspace.index()}|${pictureOf(workspace)?.get_uri() ?? ''}|${colourOf(workspace) ?? ''}|` +
                `${config.gnomeBackground}|${monitor.x},${monitor.y},${monitor.width},${monitor.height}`;
            const shown = this._shown.get(index);
            if (shown?.key === key)
                return;
            shown?.actor?.destroy();
            const actor = backgroundActor(workspace);
            if (actor) {
                actor.set_position(monitor.x, monitor.y);
                actor.set_size(monitor.width, monitor.height);
                this.layer.add_child(actor);
            }
            this._shown.set(index, {key, actor});
        });
        // Apps started now begin in the folder of the workspace the keyboard is on, or where the shell began
        this._chdir(folderOf(global.workspace_manager.get_active_workspace()) ?? this._startedIn);
    }

    // The shell's working folder, which the apps it starts inherit; GLib's own call, where it has one
    _chdir(folder) {
        if (!folder || folder === this._folder || typeof GLib.chdir !== 'function')
            return;
        if (GLib.chdir(folder) === 0)
            this._folder = folder;
        this._tellTerminal();
    }

    // Super+Return starts from gnome-settings-daemon, which this folder does not reach: its command reads it from a file,
    // there only while the workspace on screen has a folder
    _tellTerminal() {
        const file = Gio.File.new_for_path(
            GLib.build_filenamev([GLib.get_user_runtime_dir(), 'amethystora', 'workspace-folder']));
        try {
            if (this._folder === this._startedIn) {
                file.delete(null);
            } else {
                GLib.mkdir_with_parents(file.get_parent().get_path(), 0o700);
                GLib.file_set_contents(file.get_path(), this._folder);
            }
        } catch (e) {
            if (!e.matches(Gio.IOErrorEnum, Gio.IOErrorEnum.NOT_FOUND))
                console.warn(`Amethystora UI: the terminal cannot be told this workspace's folder: ${e.message}`);
        }
    }
}
