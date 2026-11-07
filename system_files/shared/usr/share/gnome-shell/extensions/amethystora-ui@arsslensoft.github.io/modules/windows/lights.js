// The window buttons as the three lights on the terminal too. GTK and libadwaita draw theirs from the stylesheets
// the theme writes (themed/window-controls-gtk*.css.tpl), but kitty draws its own title bar, a surface of its own above
// the terminal, with buttons no stylesheet reaches and no setting of its own changes. So the shell draws the lights
// over kitty's buttons: a strip of the title bar's colour (the theme's background, as kitty-theme.conf.tpl gives
// kitty) with close, minimize and maximize in Amethystora's red, amber and green, in the order of GNOME's button
// layout, their glyphs shown while the pointer is over them and grey while the window is in the background.
//
// The strip is a child of the window's own actor, so that it moves, stacks, fades and minimizes with the window and
// shows in its copies (the overview, the switcher) without being told. It is placed again only when the window's size
// changes; a kitty without a title bar (hide_window_decorations, fullscreen, the drop-down terminal) gets none.

import Clutter from 'gi://Clutter';
import GLib from 'gi://GLib';
import Gio from 'gi://Gio';
import Meta from 'gi://Meta';
import St from 'gi://St';

import {gettext as _} from 'resource:///org/gnome/shell/extensions/extension.js';

// kitty's three buttons are each as wide as its title bar is high; the strip covers them and a little more
const SPARE = 4;
const PROGRAMS = ['/usr/bin/kitty'];
const BUTTONS = {
    close: {label: () => _('Close'), icon: 'window-close-symbolic'},
    minimize: {label: () => _('Minimize'), icon: 'window-minimize-symbolic'},
    maximize: {label: () => _('Maximize'), icon: 'window-maximize-symbolic'},
};

// The window's program, by what its process runs (kitty takes any --class it is given)
const isKitty = window => {
    const pid = window.get_pid();
    if (pid <= 0 || window.get_client_type() !== Meta.WindowClientType.WAYLAND)
        return false;
    try {
        return PROGRAMS.includes(GLib.file_read_link(`/proc/${pid}/exe`));
    } catch {
        return false;
    }
};

// The buttons GNOME's layout names, in its order, whichever side it puts them on
const order = layout => {
    const names = layout.split(/[:,]/).filter(name => name in BUTTONS);
    return names.length ? names : ['minimize', 'maximize', 'close'];
};

class Strip {
    constructor(part, window) {
        this.window = window;
        this.actor = window.get_compositor_private();
        this.box = new St.BoxLayout({style_class: 'amethystora-windows-lights', reactive: true, track_hover: true});
        for (const name of part.order) {
            const button = new St.Button({
                style_class: `amethystora-windows-light amethystora-windows-light-${name}`, can_focus: false,
                track_hover: true, x_expand: true, y_expand: true, accessible_name: BUTTONS[name].label(),
                child: new St.Icon({
                    icon_name: BUTTONS[name].icon, style_class: 'amethystora-windows-light-glyph',
                    x_align: Clutter.ActorAlign.CENTER, y_align: Clutter.ActorAlign.CENTER,
                }),
            });
            button.connect('clicked', () => this._press(name));
            this.box.add_child(button);
        }
        this.actor.add_child(this.box);
        const sync = () => this.sync();
        window.connectObject('size-changed', sync, 'notify::fullscreen', sync, 'notify::appears-focused',
            () => this._focus(), this);
        this._focus();
        this.sync();
    }

    // For tests: the title bar's surface whose buttons are clipped off
    get clipped() {
        return this._clipped ?? null;
    }

    destroy() {
        this.window.disconnectObject(this);
        this._unclip();
        this.box.destroy();
    }

    _press(name) {
        const window = this.window;
        if (name === 'close')
            window.delete(global.get_current_time());
        else if (name === 'minimize')
            window.minimize();
        else if (window.maximized_horizontally && window.maximized_vertically)
            window.unmaximize();
        else
            window.maximize();
    }

    _focus() {
        if (this.window.appears_focused)
            this.box.remove_style_class_name('backdrop');
        else
            this.box.add_style_class_name('backdrop');
    }

    // Over the right end of kitty's title bar: the part of the frame above the terminal's own surface, which the
    // actor's origin is. kitty's buttons under it are clipped off its title bar's surface rather than only covered:
    // the glass draws a window in the background see-through, and they would show through the strip
    sync() {
        const f = this.window.get_frame_rect();
        const b = this.window.get_buffer_rect();
        const height = b.y - f.y;
        if (height <= 0 || this.window.is_fullscreen()) {
            this._unclip();
            this.box.hide();
            return;
        }
        const width = Math.min(f.width, 3 * height + SPARE * St.ThemeContext.get_for_stage(global.stage).scale_factor);
        const x = f.x + f.width - b.x - width;
        this.box.set_position(x, f.y - b.y);
        this.box.set_size(width, height);
        this.box.show();
        const bar = this._bar(height);
        if (bar !== this._clipped)
            this._unclip();
        bar?.set_clip(0, 0, Math.max(0, x - bar.x), bar.height);
        if (bar && bar !== this._clipped)
            bar.connectObject('destroy', () => (this._clipped = null), this);
        this._clipped = bar;
    }

    // The title bar's surface: kitty's surface right above the terminal's, as high as the title bar
    _bar(height) {
        const container = this.actor.get_first_child();
        return container?.get_children().find(child => Math.abs(child.y + height) < 1 &&
            Math.abs(child.height - height) < 1 && Math.abs(child.x) < 1) ?? null;
    }

    _unclip() {
        this._clipped?.disconnectObject(this);
        this._clipped?.remove_clip();
        this._clipped = null;
    }
}

export class Lights {
    constructor(module) {
        this._module = module;
    }

    enable() {
        this._strips = new Map();
        this._wm = new Gio.Settings({schema_id: 'org.gnome.desktop.wm.preferences'});
        this.order = order(this._wm.get_string('button-layout'));
        this._wm.connectObject('changed::button-layout', () => {
            this.order = order(this._wm.get_string('button-layout'));
            for (const window of [...this._strips.keys()]) {
                this._drop(window);
                this._consider(window);
            }
        }, this);
        global.display.connectObject('window-created', (display, window) => this._created(window), this);
        for (const actor of global.get_window_actors())
            this._consider(actor.meta_window);
    }

    disable() {
        global.display.disconnectObject(this);
        for (const window of [...(this._strips?.keys() ?? [])])
            this._drop(window);
        this._strips = null;
        this._wm?.disconnectObject(this);
        // Disposed now rather than by the collector (dock.js says why)
        this._wm?.run_dispose();
        this._wm = null;
    }

    // For tests
    get windows() {
        return [...(this._strips?.keys() ?? [])];
    }

    strip(window) {
        return this._strips?.get(window) ?? null;
    }

    // A new window has its actor and its title bar only once it is shown
    _created(window) {
        window.connectObject('shown', () => {
            window.disconnectObject(this);
            this._consider(window);
        }, this);
    }

    _consider(window) {
        if (!this._strips || this._strips.has(window) || !window.get_compositor_private() || !isKitty(window))
            return;
        this._strips.set(window, new Strip(this, window));
        window.connectObject('unmanaging', () => this._drop(window), this);
    }

    _drop(window) {
        window.disconnectObject(this);
        this._strips?.get(window)?.destroy();
        this._strips?.delete(window);
    }
}
