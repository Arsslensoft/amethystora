// Keep on top, visibly: Super+Alt+P keeps the window in use above the rest, and again lets it go. A window kept above,
// whichever way (this key, or Always on Top in GNOME's window menu), shows it: a thin ring in the accent with a soft
// glow, and a pin on its top corner, a click on which lets it go. GNOME keeps the state; this only draws it.
//
// The ring is drawn by the shell round the window, directly above the window's actor in the window group (and the
// glow directly below it, where the window hides all of it but its edge), so that a window stacked over it covers it.
// It follows the window's frame and its picture's moves, and hides with it. Nothing runs while nothing changes.

import Clutter from 'gi://Clutter';
import Gio from 'gi://Gio';
import GObject from 'gi://GObject';
import Meta from 'gi://Meta';
import Shell from 'gi://Shell';
import St from 'gi://St';

import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import {gettext as _} from 'resource:///org/gnome/shell/extensions/extension.js';

// How far the ring stands out of the frame, and the pin's size (the board's)
const OUT = 2;
const PIN = 26;
const TYPES = [Meta.WindowType.NORMAL, Meta.WindowType.DIALOG, Meta.WindowType.MODAL_DIALOG, Meta.WindowType.UTILITY];

const scale = () => St.ThemeContext.get_for_stage(global.stage).scale_factor;
const rtl = () => Clutter.get_default_text_direction() === Clutter.TextDirection.RTL;

class Ring {
    constructor(part, window) {
        this._part = part;
        this.window = window;
        this.actor = window.get_compositor_private();
        this.glow = new St.Widget({style_class: 'amethystora-windows-ontop-glow', reactive: false});
        this.ring = new St.Widget({style_class: 'amethystora-windows-ontop', reactive: false});
        this.pin = new St.Button({
            style_class: 'amethystora-windows-ontop-pin', reactive: true, can_focus: false, track_hover: true,
            accessible_name: _('Stop keeping on top'),
            child: new St.Icon({gicon: part.pinIcon, style_class: 'amethystora-windows-ontop-pin-icon'}),
        });
        this.pin.connect('clicked', () => window.unmake_above());
        this.ring.add_child(this.pin);
        const parent = this.actor.get_parent();
        parent.add_child(this.glow);
        parent.add_child(this.ring);
        // Unbound when the ring goes, not when the collector gets to it
        this._bindings = [];
        for (const each of [this.glow, this.ring]) {
            for (const name of ['translation-x', 'translation-y'])
                this._bindings.push(this.actor.bind_property(name, each, name, GObject.BindingFlags.SYNC_CREATE));
        }
        const sync = () => this.sync();
        window.connectObject('position-changed', sync, 'size-changed', sync, 'notify::fullscreen', sync,
            'notify::minimized', sync, 'workspace-changed', sync, this);
        this.actor.connectObject('notify::visible', sync, 'effects-completed', sync, this);
        this.restack();
        this.sync();
    }

    // Drawn only where the window is: shown, not fullscreen, not on its way into the dock
    sync() {
        const {window, actor} = this;
        if (!this.ring)
            return;
        const shown = actor.visible && !window.minimized && !window.is_fullscreen() && !this.hidden &&
            !this._part.module.ext.modules?.get('motion')?.flying?.(actor);
        // A see-through window would show its glow's fill: it keeps the ring only
        const module = this._part.module;
        const solid = !module.running('seeThrough') || module.seeThrough.level(window) >= 100;
        this.glow.visible = shown && solid;
        this.ring.visible = shown;
        if (!shown)
            return;
        const s = scale();
        const out = OUT * s;
        const f = window.get_frame_rect();
        const [x, y, w, h] = [f.x - out, f.y - out, f.width + 2 * out, f.height + 2 * out];
        for (const each of [this.glow, this.ring]) {
            each.set_position(x, y);
            each.set_size(w, h);
        }
        // Centred on the frame's top corner, on the side of the window's own controls
        const pin = PIN * s;
        this.pin.set_size(pin, pin);
        this.pin.set_position(Math.round((rtl() ? out : w - out) - pin / 2), Math.round(out - pin / 2));
    }

    // The glow directly under the window, the ring directly over it
    restack() {
        const parent = this.actor?.get_parent();
        if (!parent || this.ring?.get_parent() !== parent)
            return;
        parent.set_child_below_sibling(this.glow, this.actor);
        parent.set_child_above_sibling(this.ring, this.actor);
    }

    destroy() {
        for (const binding of this._bindings ?? [])
            binding.unbind();
        this._bindings = null;
        this.window?.disconnectObject(this);
        this.actor?.disconnectObject(this);
        this.glow?.destroy();
        this.ring?.destroy();
        this.glow = this.ring = this.pin = null;
        this.window = this.actor = null;
    }
}

export class OnTop {
    constructor(module) {
        this.module = module;
        this._ext = module.ext;
    }

    enable() {
        this.pinIcon = Gio.FileIcon.new(this._ext.dir.get_child('icons').get_child('screenshot')
            .get_child('pin-symbolic.svg'));
        this._rings = new Map();
        this._watched = new Set();
        Main.wm.addKeybinding('keep-on-top-key', this.module.settings, Meta.KeyBindingFlags.IGNORE_AUTOREPEAT,
            Shell.ActionMode.NORMAL, () => this.toggleFocused());
        global.display.connectObject('window-created', (d, window) => this._watch(window),
            'restacked', () => {
                for (const ring of this._rings.values())
                    ring.restack();
            }, this);
        // A window going into the dock leaves its ring at once; the ring comes back when the window does
        global.window_manager.connectObject('minimize', (wm, actor) => {
            const ring = this._rings.get(actor.meta_window);
            if (ring) {
                ring.glow.hide();
                ring.ring.hide();
            }
        }, this);
        for (const actor of global.get_window_actors())
            this._watch(actor.meta_window);
    }

    disable() {
        Main.wm.removeKeybinding('keep-on-top-key');
        global.display.disconnectObject(this);
        global.window_manager.disconnectObject(this);
        for (const window of this._watched ?? [])
            window.disconnectObject(this);
        this._watched = null;
        for (const ring of this._rings?.values() ?? [])
            ring.destroy();
        this._rings = null;
        this.pinIcon = null;
    }

    // For tests
    ringOf(window) {
        return this._rings?.get(window) ?? null;
    }

    get rings() {
        return this._rings?.size ?? 0;
    }

    // The key: the window in use above the rest, or let go
    toggleFocused() {
        const window = global.display.focus_window;
        if (!window || !TYPES.includes(window.get_window_type()) || this.module.holds(window))
            return;
        if (window.is_above())
            window.unmake_above();
        else if (!window.is_fullscreen())
            window.make_above();
    }

    _watch(window) {
        if (!window || this._watched.has(window) || !TYPES.includes(window.get_window_type()))
            return;
        this._watched.add(window);
        window.connectObject('notify::above', () => this._sync(window), 'unmanaging', () => {
            window.disconnectObject(this);
            this._watched?.delete(window);
            this._rings?.get(window)?.destroy();
            this._rings?.delete(window);
        }, this);
        // A new window has no picture yet: its ring, if it opens above, waits for it
        if (window.get_compositor_private())
            this._sync(window);
        else
            window.connectObject('shown', () => this._sync(window), this);
    }

    _sync(window) {
        if (!this._rings)
            return;
        const wanted = window.is_above() && !this.module.holds(window) && !!window.get_compositor_private()?.get_parent();
        const ring = this._rings.get(window);
        if (wanted && !ring)
            this._rings.set(window, new Ring(this, window));
        else if (!wanted && ring) {
            ring.destroy();
            this._rings.delete(window);
        }
    }

    // The scratchpad took or gave back a window: its ring follows
    resync(window) {
        if (window && this._watched?.has(window))
            this._sync(window);
    }
}
