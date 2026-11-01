// Hot corners: each corner of the screen can do one thing when the pointer is pushed into it (the overview, show the
// desktop, lock the screen, a quick note, the notifications), and all four do nothing until the person chooses. A
// corner fires as GNOME's own does, after a short push past the edge, so a pointer passing by never sets it off; it
// fires again only once the pointer has left it. A corner that touches another screen has none, and GNOME's own corner,
// when it is turned on, keeps its place.

import Clutter from 'gi://Clutter';
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import Meta from 'gi://Meta';
import Shell from 'gi://Shell';
import St from 'gi://St';

import * as Main from 'resource:///org/gnome/shell/ui/main.js';

// GNOME's own feel: this much push within this long
const THRESHOLD = 100;
const TIMEOUT = 1000;
const GLOW = 180;
const GLOW_FADE = 240;

// A push into a corner: what each hit of either of its two barriers crosses, added up over the last TIMEOUT ms. GNOME's
// PressureBarrier leaves out a hit that moves along its barrier as far as across it, which a push straight into a
// corner always nearly does, and on a right edge the pointer stopped at the screen's last pixel reaches only the
// barrier along the top: half of a diagonal push was lost there, and the top-right corner did not fire. A pointer
// gliding along an edge adds nothing, and one drifting in slowly too little.
class CornerPressure {
    constructor(threshold, timeout, modes, onTrigger) {
        this._threshold = threshold;
        this._timeout = timeout;
        this._modes = modes;
        this._onTrigger = onTrigger;
        this._barriers = new Set();
        this._held = new Set();
        this._events = [];
        this._triggered = false;
    }

    addBarrier(barrier) {
        this._barriers.add(barrier);
        barrier.connectObject('hit', (b, event) => this._hit(b, event), 'left', b => this._left(b), this);
    }

    removeBarrier(barrier) {
        barrier.disconnectObject(this);
        this._barriers.delete(barrier);
        this._held.delete(barrier);
    }

    destroy() {
        for (const barrier of [...this._barriers])
            this.removeBarrier(barrier);
        this._events = [];
        this._onTrigger = null;
    }

    _hit(barrier, event) {
        this._held.add(barrier);
        // Once fired, a corner waits for the pointer to leave it
        if (this._triggered || !(this._modes & Main.actionMode))
            return;
        const vertical = barrier.x1 === barrier.x2;
        const across = Math.abs(vertical ? event.dx : event.dy);
        const along = Math.abs(vertical ? event.dy : event.dx);
        if (along > 2 * across)
            return;
        this._events = this._events.filter(([time]) => event.time - time < this._timeout);
        this._events.push([event.time, Math.min(15, across)]);
        if (this._events.reduce((sum, [, push]) => sum + push, 0) < this._threshold)
            return;
        this._triggered = true;
        this._events = [];
        this._onTrigger?.();
    }

    _left(barrier) {
        this._held.delete(barrier);
        if (this._held.size)
            return;
        this._triggered = false;
        this._events = [];
    }
}

export const CORNERS = ['top-left', 'top-right', 'bottom-left', 'bottom-right'];
export const ACTIONS = ['none', 'overview', 'desktop', 'lock', 'note', 'notifications'];

export class Corners {
    constructor(module) {
        this._module = module;
        this._ext = module.ext;
        this._built = [];
        this._glows = new Map();
    }

    enable() {
        this._interface = new Gio.Settings({schema_id: 'org.gnome.desktop.interface'});
        this._lockdown = new Gio.Settings({schema_id: 'org.gnome.desktop.lockdown'});
        this._interface.connectObject('changed::enable-hot-corners', () => this.rebuild(), this);
        Main.layoutManager.connectObject('monitors-changed', () => this.rebuild(), this);
        Main.panel.connectObject('notify::height', () => this.rebuild(), this);
        this.rebuild();
    }

    disable() {
        this._interface?.disconnectObject(this);
        Main.layoutManager.disconnectObject(this);
        Main.panel.disconnectObject(this);
        this._destroy();
        for (const glow of this._glows.values())
            glow.destroy();
        this._glows.clear();
        // Disposed now rather than by the collector (dock.js says why)
        this._interface?.run_dispose();
        this._interface = null;
        this._lockdown?.run_dispose();
        this._lockdown = null;
    }

    get barrierCount() {
        return this._built.reduce((sum, corner) => sum + corner.barriers.length, 0);
    }

    _destroy() {
        for (const corner of this._built) {
            for (const barrier of corner.barriers) {
                corner.pressure.removeBarrier(barrier);
                barrier.destroy();
            }
            corner.pressure.destroy();
        }
        this._built = [];
    }

    // Whether a monitor has this corner: no other monitor directly beside it or above or below it there
    _free(monitor, where) {
        const left = where.endsWith('left');
        const top = where.startsWith('top');
        const x = left ? monitor.x : monitor.x + monitor.width;
        const y = top ? monitor.y : monitor.y + monitor.height;
        const beside = [left ? x - 1 : x, top ? y : y - 1];
        const across = [left ? x : x - 1, top ? y - 1 : y];
        return !Main.layoutManager.monitors.some(other => other !== monitor &&
            [beside, across].some(([px, py]) => px >= other.x && px < other.x + other.width &&
                py >= other.y && py < other.y + other.height));
    }

    rebuild() {
        this._destroy();
        const size = Main.panel.height;
        if (size <= 0)
            return;
        const rtl = Clutter.get_default_text_direction() === Clutter.TextDirection.RTL;
        const gnomeCorner = this._interface.get_boolean('enable-hot-corners') ? (rtl ? 'top-right' : 'top-left') : null;
        for (const where of CORNERS) {
            const action = this._module.settings.get_string(`corner-${where}`);
            if (action === 'none' || !ACTIONS.includes(action) || where === gnomeCorner)
                continue;
            for (const monitor of Main.layoutManager.monitors) {
                if (!this._free(monitor, where))
                    continue;
                this._build(monitor, where, action, size);
            }
        }
    }

    _build(monitor, where, action, size) {
        const left = where.endsWith('left');
        const top = where.startsWith('top');
        const x = left ? monitor.x : monitor.x + monitor.width;
        const y = top ? monitor.y : monitor.y + monitor.height;
        const modes = ['overview', 'notifications'].includes(action)
            ? Shell.ActionMode.NORMAL | Shell.ActionMode.OVERVIEW : Shell.ActionMode.NORMAL;
        const pressure = new CornerPressure(THRESHOLD, TIMEOUT, modes, () => this._fire(monitor, where, action, x, y));
        // Two barriers along the edges that meet there, each holding back a push out of the screen
        const vertical = new Meta.Barrier({
            backend: global.backend, x1: x, x2: x,
            y1: top ? y : y - size, y2: top ? y + size : y,
            directions: left ? Meta.BarrierDirection.POSITIVE_X : Meta.BarrierDirection.NEGATIVE_X,
        });
        const horizontal = new Meta.Barrier({
            backend: global.backend, y1: y, y2: y,
            x1: left ? x : x - size, x2: left ? x + size : x,
            directions: top ? Meta.BarrierDirection.POSITIVE_Y : Meta.BarrierDirection.NEGATIVE_Y,
        });
        pressure.addBarrier(vertical);
        pressure.addBarrier(horizontal);
        this._built.push({monitor, where, action, pressure, barriers: [vertical, horizontal]});
    }

    _fire(monitor, where, action, x, y) {
        this._module.count('cornerFired');
        // Never over a fullscreen window, unless the overview is open over it
        if (monitor.inFullscreen && !Main.overview.visible)
            return;
        // Locking the screen turns the extension off before run() returns: no glow then, which nothing would take away
        if (this.run(action) && this._interface)
            this._glow(monitor, where, x, y);
    }

    // What a corner does; true when it did something
    run(action) {
        switch (action) {
        case 'overview':
            if (!Main.overview.shouldToggleByCornerOrButton())
                return false;
            Main.overview.toggle();
            return true;
        case 'desktop':
            if (!this._module.classic)
                return false;
            this._module.desktop.toggle();
            return true;
        case 'lock':
            if (this._lockdown.get_boolean('disable-lock-screen'))
                return false;
            Main.screenShield.lock(true);
            return true;
        case 'note':
            return this._note();
        case 'notifications':
            Main.panel.toggleCalendar();
            return true;
        }
        return false;
    }

    // The quick note, once the Notes command takes it; until then the corner does nothing
    _note() {
        const path = GLib.find_program_in_path('amethystora-notes');
        if (!path)
            return false;
        try {
            const [, bytes] = GLib.file_get_contents(path);
            if (!new TextDecoder().decode(bytes).includes('capture'))
                return false;
            Gio.Subprocess.new(['amethystora-notes', 'capture'], Gio.SubprocessFlags.NONE);
            return true;
        } catch (e) {
            console.warn(`Amethystora UI: the quick note did not open: ${e.message}`);
            return false;
        }
    }

    // A glow of the accent at the corner, faded out
    _glow(monitor, where, x, y) {
        const key = `${monitor.index}:${where}`;
        let glow = this._glows.get(key);
        const s = St.ThemeContext.get_for_stage(global.stage).scale_factor;
        if (!glow) {
            glow = new St.Widget({style_class: 'amethystora-windows-glow', reactive: false});
            Main.layoutManager.uiGroup.add_child(glow);
            this._glows.set(key, glow);
        }
        glow.set_position(x - GLOW * s / 2, y - GLOW * s / 2);
        Main.layoutManager.uiGroup.set_child_above_sibling(glow, null);
        glow.remove_all_transitions();
        glow.opacity = 255;
        glow.show();
        glow.ease({opacity: 0, duration: GLOW_FADE, mode: Clutter.AnimationMode.EASE,
            onComplete: () => glow.hide()});
    }
}
