// Daylight's night picture while a profile asks for it: the current theme's own picture for the night (the night line
// of its daylight.list), drawn over GNOME's wallpaper on every screen, above a workspace's own picture, and faded in and
// out. GNOME's wallpaper setting is not written, so a crash leaves nothing behind. One picture per screen while on.

import Clutter from 'gi://Clutter';
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import Meta from 'gi://Meta';
import St from 'gi://St';

import * as Main from 'resource:///org/gnome/shell/ui/main.js';

const LIST = GLib.build_filenamev([GLib.get_user_config_dir(), 'amethystora', 'current', 'theme', 'daylight.list']);
const PICTURE = /\.(jpg|jpeg|png|webp|svg)$/i;
const FADE = 300;

// The night picture as amethystora-theme reads the list: four lines, dawn, day, dusk and night, each an absolute path
// to a picture that is a regular file; anything else is no picture
export function nightPicture() {
    let text;
    try {
        const [, bytes] = GLib.file_get_contents(LIST);
        if (bytes.length > 8192)
            return null;
        text = new TextDecoder().decode(bytes);
    } catch {
        return null;
    }
    const lines = text.split('\n').map(line => line.replace(/\r$/, '')).filter(line => line !== '');
    const words = ['dawn', 'day', 'dusk', 'night'];
    if (lines.length !== 4)
        return null;
    let night = null;
    for (const [i, line] of lines.entries()) {
        const space = line.indexOf(' ');
        const word = line.slice(0, space);
        const path = line.slice(space + 1);
        if (space < 0 || word !== words[i] || !path.startsWith('/') || !PICTURE.test(path) ||
            !GLib.file_test(path, GLib.FileTest.IS_REGULAR))
            return null;
        if (word === 'night')
            night = path;
    }
    return night;
}

export class NightPicture {
    constructor(ext, reduce) {
        this._ext = ext;
        this._reduce = reduce;
        this._layer = null;
        this.on = false;
    }

    get shown() {
        return this._layer?.get_n_children() ?? 0;
    }

    set(on) {
        if (on === this.on)
            return;
        this.on = on;
        if (on) {
            // A theme switch while it is on brings that theme's night picture (or none)
            St.ThemeContext.get_for_stage(global.stage).connectObject('changed', () => this._themeChanged(), this);
            this._show();
        } else {
            St.ThemeContext.get_for_stage(global.stage).disconnectObject(this);
            this._hide();
        }
    }

    stop() {
        this.on = false;
        St.ThemeContext.get_for_stage(global.stage).disconnectObject(this);
        Main.layoutManager.disconnectObject(this);
        this._forgetRestacks();
        this._layer?.destroy();
        this._layer = null;
    }

    _themeChanged() {
        const path = nightPicture();
        if (path === (this._layer ? this._path : null))
            return;
        if (path)
            this._show();
        else
            this._hide();
    }

    _show() {
        const path = nightPicture();
        if (!path)
            return;
        this._path = path;
        if (!this._layer) {
            this._layer = new Clutter.Actor({reactive: false, opacity: 0});
            global.window_group.add_child(this._layer);
            this._lower();
            Main.layoutManager.connectObject('monitors-changed', () => this._fill(), this);
            // A restack lowers the windows under every other actor of their group: the picture goes back under them,
            // once the workspace pictures have gone back first
            global.display.connectObject('restacked', () => {
                this._later ||= global.compositor.get_laters().add(Meta.LaterType.BEFORE_REDRAW, () => {
                    this._later = 0;
                    this._lower();
                    return false;
                });
            }, this);
        }
        this._fill();
        this._layer.remove_transition('opacity');
        this._layer.ease({
            opacity: 255,
            duration: this._reduce() ? 0 : FADE,
            mode: Clutter.AnimationMode.EASE_OUT_QUAD,
        });
    }

    // Above GNOME's wallpaper and a workspace's own picture, under every window
    _lower() {
        const group = Main.layoutManager._backgroundGroup;
        const above = this._ext.modules?.get('tiling')?.engine?.backgrounds?.layer ?? group;
        if (above?.get_parent() === global.window_group)
            global.window_group.set_child_above_sibling(this._layer, above);
        else
            global.window_group.set_child_below_sibling(this._layer, null);
    }

    _forgetRestacks() {
        global.display.disconnectObject(this);
        if (this._later)
            global.compositor.get_laters().remove(this._later);
        this._later = 0;
    }

    _hide() {
        const layer = this._layer;
        if (!layer)
            return;
        Main.layoutManager.disconnectObject(this);
        this._forgetRestacks();
        this._layer = null;
        layer.remove_transition('opacity');
        layer.ease({
            opacity: 0,
            duration: this._reduce() ? 0 : FADE,
            mode: Clutter.AnimationMode.EASE_OUT_QUAD,
            onStopped: () => layer.destroy(),
        });
    }

    // Each screen covered, as GNOME's zoom does, by the shell's own loading of pictures
    _fill() {
        const layer = this._layer;
        if (!layer)
            return;
        layer.destroy_all_children();
        const uri = Gio.File.new_for_path(this._path).get_uri().replace(/["\\]/g, c => `\\${c}`);
        for (const monitor of Main.layoutManager.monitors) {
            const picture = new St.Widget({
                reactive: false,
                style: `background-image: url("${uri}"); background-size: cover;`,
                x: monitor.x,
                y: monitor.y,
                width: monitor.width,
                height: monitor.height,
            });
            layer.add_child(picture);
        }
    }
}
