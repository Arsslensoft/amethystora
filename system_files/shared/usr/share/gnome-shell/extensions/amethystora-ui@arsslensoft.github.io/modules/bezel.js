// Rounded screen corners, for a laptop whose own screen is square: a small black curve in each corner of the
// built-in panel, over everything, so that the whole screen reads as one rounded piece of glass. Other screens
// (a monitor on a cable, a desktop's) stay square.
//
// Four tiny shapes, drawn once and never again until the screens or their scale change, which take no clicks and
// sit above everything else the shell draws. Off (ame desktop screen-corners off, the image's default), the module
// holds nothing and listens to its one key only.
//
// An actor over a fullscreen window keeps Mutter from handing that window to the screen directly, so a fullscreen
// game or video costs one more composite while the corners are on; the manual says so.

import Cairo from 'cairo';
import GObject from 'gi://GObject';
import St from 'gi://St';

import * as Main from 'resource:///org/gnome/shell/ui/main.js';

const SCHEMA = 'org.gnome.shell.extensions.amethystora-ui.bezel';
const KEY = 'round-corners';
// The radius, in logical pixels
const RADIUS = 14;

// One corner: black outside a quarter circle centred on the corner's inner point. `flipX` and `flipY` say which
// corner, so that one drawing serves all four.
const Corner = GObject.registerClass(
class AmethystoraBezelCorner extends St.DrawingArea {
    _init(flipX, flipY, size) {
        super._init({
            style_class: 'amethystora-bezel-corner',
            reactive: false,
            can_focus: false,
            track_hover: false,
            width: size,
            height: size,
        });
        this._flipX = flipX;
        this._flipY = flipY;
    }

    vfunc_repaint() {
        const cr = this.get_context();
        const [width, height] = this.get_surface_size();
        // The drawing's own units, the radius one, whatever the actor's size in pixels
        cr.scale(width, height);
        if (this._flipX) {
            cr.translate(1, 0);
            cr.scale(-1, 1);
        }
        if (this._flipY) {
            cr.translate(0, 1);
            cr.scale(1, -1);
        }
        cr.setOperator(Cairo.Operator.SOURCE);
        cr.setSourceRGBA(0, 0, 0, 1);
        // From the corner along the top edge, round the arc to the side edge, and back to the corner
        cr.moveTo(0, 0);
        cr.lineTo(1, 0);
        cr.arcNegative(1, 1, 1, -Math.PI / 2, Math.PI);
        cr.closePath();
        cr.fill();
        cr.$dispose();
    }
});

export class Bezel {
    constructor(ext) {
        this._ext = ext;
    }

    enable() {
        this._corners = [];
        this._settings = this._ext.getSettings(SCHEMA);
        this._settings.connectObject(`changed::${KEY}`, () => this._sync(), this);
        this._sync();
    }

    disable() {
        this._stop();
        this._settings?.disconnectObject(this);
        // Disposed now rather than by the collector (dock.js says why)
        this._settings?.run_dispose();
        this._settings = null;
        this._corners = null;
    }

    _sync() {
        if (this._settings.get_boolean(KEY))
            this._start();
        else
            this._stop();
    }

    _start() {
        if (this._on)
            return;
        this._on = true;
        Main.layoutManager.connectObject('monitors-changed', () => this._build(), this);
        St.ThemeContext.get_for_stage(global.stage).connectObject('notify::scale-factor', () => this._build(), this);
        // Whatever the shell adds later (a menu, the screenshot tool) may go on top of the uiGroup: the corners go back
        // above it. Most of what it adds (a workspace switch, the window switcher) goes below them, and then nothing
        // moves, since every move lays the uiGroup out again.
        Main.uiGroup.connectObject('child-added', (group, child) => {
            if (!this._corners.includes(child) && !this._onTop())
                this._raise();
        }, this);
        this._build();
    }

    _stop() {
        if (!this._on)
            return;
        this._on = false;
        Main.layoutManager.disconnectObject(this);
        St.ThemeContext.get_for_stage(global.stage).disconnectObject(this);
        Main.uiGroup.disconnectObject(this);
        this._clear();
    }

    _clear() {
        for (const corner of this._corners)
            corner.destroy();
        this._corners = [];
    }

    // The monitors whose panel is the computer's own, as layout monitors
    _builtinMonitors() {
        const manager = global.backend.get_monitor_manager();
        const indexes = new Set();
        for (const monitor of manager.get_monitors()) {
            if (!monitor.is_builtin())
                continue;
            const index = manager.get_monitor_for_connector(monitor.get_connector());
            if (index >= 0 && Main.layoutManager.monitors[index])
                indexes.add(index);
        }
        return [...indexes].map(i => Main.layoutManager.monitors[i]);
    }

    _build() {
        this._clear();
        const size = Math.round(RADIUS * St.ThemeContext.get_for_stage(global.stage).scale_factor);
        for (const monitor of this._builtinMonitors()) {
            for (const [flipX, flipY] of [[false, false], [true, false], [false, true], [true, true]]) {
                const corner = new Corner(flipX, flipY, size);
                corner.set_position(
                    flipX ? monitor.x + monitor.width - size : monitor.x,
                    flipY ? monitor.y + monitor.height - size : monitor.y);
                this._corners.push(corner);
                // Over fullscreen windows too; not reactive, so a click there reaches what is under it
                Main.layoutManager.addTopChrome(corner, {trackFullscreen: false});
            }
        }
        this._raise();
    }

    // Whether the corners are the uiGroup's last children, in their order
    _onTop() {
        let actor = Main.uiGroup.get_last_child();
        for (let i = this._corners.length - 1; i >= 0; i--) {
            if (actor !== this._corners[i])
                return false;
            actor = actor.get_previous_sibling();
        }
        return true;
    }

    _raise() {
        for (const corner of this._corners)
            Main.uiGroup.set_child_above_sibling(corner, null);
    }
}
