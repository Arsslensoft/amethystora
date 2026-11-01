// One dock on one screen (spec 1.15): its strip along the chosen edge, kept free of windows unless it hides when
// touched, and the row of icons in it. The strip takes the whole edge, so that the window manager keeps all of it
// free; only the dock in it takes the pointer.

import Clutter from 'gi://Clutter';
import St from 'gi://St';

import * as Main from 'resource:///org/gnome/shell/ui/main.js';

import {Autohide} from './autohide.js';
import {DockDash} from './dash.js';
import {EDGES} from './util.js';

// The longest the dock grows, as a share of the screen's free length
const LENGTH = 0.9;

// Whether another screen continues past the edge of monitor, overlapping it along that edge
export function sharedEdge(monitor, edgeName) {
    return Main.layoutManager.monitors.some(other => {
        if (other === monitor)
            return false;
        if (edgeName === 'bottom') {
            return other.y >= monitor.y + monitor.height &&
                other.x < monitor.x + monitor.width && other.x + other.width > monitor.x;
        }
        const along = other.y < monitor.y + monitor.height && other.y + other.height > monitor.y;
        if (edgeName === 'left')
            return other.x + other.width <= monitor.x && along;
        return other.x >= monitor.x + monitor.width && along;
    });
}

export class DockSite {
    constructor(dock, monitorIndex, edgeName, autohide) {
        this._dock = dock;
        this.monitorIndex = monitorIndex;
        this.edgeName = edgeName;
        this.edge = EDGES[edgeName];
        const monitor = Main.layoutManager.monitors[monitorIndex];
        this.shared = sharedEdge(monitor, edgeName);
        // touch: hides when a window touches it; a shared edge always does, without a barrier
        this.touch = autohide === 'touch' || this.shared;
        this.barrier = autohide === 'touch' && !this.shared;
    }

    build() {
        this.strip = new St.Widget({
            name: this.monitorIndex === Main.layoutManager.primaryIndex ? 'amethystoraDock' : null,
            style_class: `amethystora-dock-strip amethystora-dock-edge-${this.edgeName}`,
            layout_manager: new Clutter.BinLayout(),
            clip_to_allocation: false,
        });
        this.dash = new DockDash(this._dock, this);
        this.strip.add_child(this.dash);
        Main.layoutManager.addChrome(this.strip, {affectsStruts: !this.touch, trackFullscreen: true});
        this.strip.connectObject('notify::height', () => this.place(), 'notify::width', () => this.place(),
            'destroy', () => {
                this.strip = null;
                this.dash = null;
            }, this);
        this.place();
        if (this.touch) {
            this.autohide = new Autohide(this, this._dock);
            this.autohide.start();
        }
    }

    destroy() {
        this.autohide?.stop();
        this.autohide = null;
        this.strip?.destroy();
        this.strip = null;
        this.dash = null;
    }

    get reserves() {
        return !this.touch;
    }

    place() {
        const monitor = Main.layoutManager.monitors[this.monitorIndex];
        if (!this.strip || !monitor || this._placing)
            return;
        this._placing = true;
        try {
            const area = Main.layoutManager.getWorkAreaForMonitor(this.monitorIndex);
            // A top bar moved to the bottom of this screen (modules/bars.js): the dock sits above it
            const inset = this._dock.ext.modules?.get('bars')?.bottomInset?.(this.monitorIndex) ?? 0;
            if (this.edgeName === 'bottom') {
                const room = Math.floor(area.width * LENGTH);
                const left = area.x - monitor.x + Math.floor((area.width - room) / 2);
                this.dash.set({
                    margin_left: left, margin_right: Math.max(0, monitor.width - left - room),
                    margin_top: 0, margin_bottom: 0,
                });
                this.dash.fitTo(room);
                this.strip.width = monitor.width;
                const [, height] = this.strip.get_preferred_height(monitor.width);
                // Its height is its own, so that it follows the icons' size
                this.strip.height = -1;
                this.strip.set_position(monitor.x, monitor.y + monitor.height - inset - height);
            } else {
                // From below the top bar to the screen's bottom, centred on the free height
                const top = area.y;
                const free = monitor.y + monitor.height - inset - top;
                const room = Math.floor(area.height * LENGTH);
                const before = Math.floor((area.height - room) / 2);
                this.dash.set({
                    margin_top: before, margin_bottom: Math.max(0, free - before - room),
                    margin_left: 0, margin_right: 0,
                });
                this.dash.fitTo(room);
                this.strip.height = free;
                const [, width] = this.strip.get_preferred_width(free);
                this.strip.width = -1;
                const x = this.edgeName === 'left' ? monitor.x : monitor.x + monitor.width - width;
                this.strip.set_position(x, top);
            }
        } finally {
            this._placing = false;
        }
        this._dock.placed(this);
    }

    // The strip on the stage, where it rests
    stripRect() {
        if (!this.strip)
            return {x: 0, y: 0, width: 0, height: 0};
        return {x: this.strip.x, y: this.strip.y, width: this.strip.width, height: this.strip.height};
    }

    get thickness() {
        if (!this.strip)
            return 0;
        return this.edgeName === 'bottom' ? this.strip.height : this.strip.width;
    }
}
