// Hiding when a window touches the dock (spec 1.15). The dock then keeps no strip of the screen free and slides past
// the edge while a window of the workspace shown on its screen overlaps it, leaving a short bar at the edge. Pushing
// the pointer against that edge brings it back (GNOME's hot corner pressure, along the dock's length); it stays while
// the pointer is on it and goes again half a second after the pointer leaves. It also shows in the overview, while it
// has the keyboard, while one of its menus or cards is open, and when the windows module asks for its numbers.
//
// Where another screen continues past the dock's edge the window manager keeps no strip free there, and a barrier would
// hold the pointer back from the next screen: such a dock hides the same way, without the barrier.

import GLib from 'gi://GLib';
import Meta from 'gi://Meta';
import Shell from 'gi://Shell';
import St from 'gi://St';

import * as Layout from 'resource:///org/gnome/shell/ui/layout.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';

import {EASE, REDUCED_FADE, scaleFactor} from './util.js';

const SLIDE = 420;
const PAST = 4;
const HINT = {length: 90, thickness: 4, out: 3, fade: 300};
const STAY = 500;
// GNOME's hot corner pressure (layout.js)
const PRESSURE = 100;
const PRESSURE_TIME = 1000;

export class Autohide {
    // site: the dock on one screen (strip, dash, monitorIndex, edgeName, barrier)
    constructor(site, dock) {
        this._site = site;
        this._dock = dock;
        this._holds = new Set();
        this.hidden = false;
        this._touched = false;
        this._windows = new Set();
    }

    start() {
        const site = this._site;
        this._hint = new St.Widget({style_class: 'amethystora-dock-hint', opacity: 0, reactive: false});
        Main.layoutManager.addTopChrome(this._hint);
        this._hint.hide();
        global.display.connectObject(
            'window-created', (display, window) => this._watch(window),
            'restacked', () => this._queue(),
            this);
        global.workspace_manager.connectObject('active-workspace-changed', () => this._queue(), this);
        for (const actor of global.get_window_actors())
            this._watch(actor.meta_window);
        Main.overview.connectObject(
            'showing', () => this.hold('overview', true),
            'hidden', () => this.hold('overview', false), this);
        site.dash.connectObject(
            'notify::hover', () => this._hovered(),
            'key-focus-in', () => this.hold('focus', true),
            'key-focus-out', () => this.hold('focus', false), this);
        global.stage.connectObject('notify::key-focus', () => {
            const focus = global.stage.key_focus;
            this.hold('focus', Boolean(focus && site.dash.contains(focus)));
        }, this);
        // Where it slides to and where the hint goes follow the dock's size
        site.strip.connectObject('notify::allocation', () => {
            if (this.hidden) {
                this._synced = false;
                this._sync();
            }
        }, this);
        if (site.barrier)
            this._makeBarrier();
        this._queue();
    }

    stop() {
        if (this._checkId)
            global.compositor.get_laters().remove(this._checkId);
        this._checkId = 0;
        if (this._stayId)
            GLib.source_remove(this._stayId);
        this._stayId = 0;
        global.display.disconnectObject(this);
        global.workspace_manager.disconnectObject(this);
        global.stage.disconnectObject(this);
        this._site.strip?.disconnectObject(this);
        Main.overview.disconnectObject(this);
        this._site.dash?.disconnectObject(this);
        for (const window of this._windows)
            window.disconnectObject(this);
        this._windows.clear();
        this._removeBarrier();
        this._hint?.destroy();
        this._hint = null;
        const strip = this._site.strip;
        if (strip) {
            strip.remove_all_transitions();
            strip.set({translation_x: 0, translation_y: 0, opacity: 255});
        }
    }

    get hasBarrier() {
        return Boolean(this._barrier);
    }

    // A reason to stay shown (overview, focus, menu, card, numbers, pressure, drag) begins or ends
    hold(reason, on) {
        if (on === this._holds.has(reason))
            return;
        if (on)
            this._holds.add(reason);
        else
            this._holds.delete(reason);
        this._sync();
    }

    _watch(window) {
        if (this._windows.has(window) || window.get_window_type() === Meta.WindowType.DESKTOP)
            return;
        this._windows.add(window);
        window.connectObject(
            'position-changed', () => this._queue(),
            'size-changed', () => this._queue(),
            'notify::minimized', () => this._queue(),
            'workspace-changed', () => this._queue(),
            'unmanaged', () => {
                window.disconnectObject(this);
                this._windows.delete(window);
                this._queue();
            }, this);
        this._queue();
    }

    // Looked at once per frame at most, however many windows moved
    _queue() {
        if (this._checkId)
            return;
        this._checkId = global.compositor.get_laters().add(Meta.LaterType.BEFORE_REDRAW, () => {
            this._checkId = 0;
            this._check();
            return GLib.SOURCE_REMOVE;
        });
    }

    _check() {
        const rect = this._site.stripRect();
        const workspace = this._dock.shownOn(this._site.monitorIndex);
        let touched = false;
        for (const window of this._windows) {
            if (window.minimized || !window.showing_on_its_workspace() || window.skip_taskbar &&
                window.get_window_type() !== Meta.WindowType.NORMAL)
                continue;
            if (!window.located_on_workspace(workspace))
                continue;
            const frame = window.get_frame_rect();
            if (frame.x < rect.x + rect.width && frame.x + frame.width > rect.x &&
                frame.y < rect.y + rect.height && frame.y + frame.height > rect.y) {
                touched = true;
                break;
            }
        }
        this._touched = touched;
        this._sync();
    }

    _hovered() {
        if (this._site.dash.hover) {
            if (this._stayId)
                GLib.source_remove(this._stayId);
            this._stayId = 0;
            return;
        }
        if (!this._holds.has('pressure') || this._stayId)
            return;
        this._stayId = GLib.timeout_add(GLib.PRIORITY_DEFAULT, STAY, () => {
            this._stayId = 0;
            if (!this._site.dash.hover)
                this.hold('pressure', false);
            return GLib.SOURCE_REMOVE;
        });
    }

    _sync() {
        const hide = this._touched && this._holds.size === 0;
        if (hide === this.hidden && this._synced)
            return;
        this._synced = true;
        this.hidden = hide;
        const site = this._site;
        const strip = site.strip;
        if (!strip)
            return;
        // Not laid out yet: looked at again once it is
        if (!strip.has_allocation() || !site.dash?.has_allocation()) {
            this._synced = false;
            this._queue();
            return;
        }
        const level = this._dock.motionLevel();
        const sf = scaleFactor();
        const thickness = site.edgeName === 'bottom' ? strip.height : strip.width;
        const past = thickness + PAST * sf;
        const away = {bottom: [0, past], left: [-past, 0], right: [past, 0]}[site.edgeName];
        const to = hide ? {translation_x: away[0], translation_y: away[1]} : {translation_x: 0, translation_y: 0};
        strip.remove_all_transitions();
        // Where the items are is read again once the strip stands still: the pointer may already be on it
        const settled = () => site.dash?.swell?.invalidate();
        if (level === 'none') {
            strip.set({...to, opacity: 255});
            settled();
        } else if (level === 'reduced') {
            // Nothing travels: it fades out where it is, and in again
            if (hide) {
                strip.ease({
                    opacity: 0, duration: REDUCED_FADE, mode: EASE.out, onComplete: () => {
                        strip.set(to);
                        settled();
                    },
                });
            } else {
                strip.set(to);
                settled();
                strip.ease({opacity: 255, duration: REDUCED_FADE, mode: EASE.out});
            }
        } else {
            strip.opacity = 255;
            strip.ease({...to, duration: SLIDE, mode: EASE.quart, onComplete: settled});
        }
        this._syncHint(hide, level);
        if (hide)
            this._site.dash.closePopups?.();
    }

    _syncHint(show, level) {
        const hint = this._hint;
        if (!hint)
            return;
        const sf = scaleFactor();
        const monitor = Main.layoutManager.monitors[this._site.monitorIndex];
        const pill = this._site.dash.pillRect();
        const along = this._site.edgeName === 'bottom' ? HINT.length : HINT.thickness;
        const across = this._site.edgeName === 'bottom' ? HINT.thickness : HINT.length;
        hint.set_size(along * sf, across * sf);
        if (this._site.edgeName === 'bottom') {
            hint.set_position(Math.round(pill.x + (pill.width - HINT.length * sf) / 2),
                monitor.y + monitor.height - (HINT.out + HINT.thickness) * sf);
        } else {
            const x = this._site.edgeName === 'left' ? monitor.x + HINT.out * sf
                : monitor.x + monitor.width - (HINT.out + HINT.thickness) * sf;
            hint.set_position(Math.round(x), Math.round(pill.y + (pill.height - HINT.length * sf) / 2));
        }
        hint.remove_all_transitions();
        if (show)
            hint.show();
        hint.ease({
            opacity: show ? 255 : 0,
            duration: level === 'none' ? 0 : HINT.fade,
            mode: EASE.out,
            onComplete: () => {
                if (!show)
                    hint.hide();
            },
        });
    }

    _makeBarrier() {
        const monitor = Main.layoutManager.monitors[this._site.monitorIndex];
        if (!monitor)
            return;
        const rect = this._site.stripRect();
        const params = {backend: global.backend};
        if (this._site.edgeName === 'bottom') {
            const y = monitor.y + monitor.height;
            Object.assign(params, {
                x1: rect.x, x2: rect.x + rect.width, y1: y, y2: y,
                directions: Meta.BarrierDirection.NEGATIVE_Y,
            });
        } else if (this._site.edgeName === 'left') {
            Object.assign(params, {
                x1: monitor.x, x2: monitor.x, y1: rect.y, y2: rect.y + rect.height,
                directions: Meta.BarrierDirection.POSITIVE_X,
            });
        } else {
            const x = monitor.x + monitor.width;
            Object.assign(params, {
                x1: x, x2: x, y1: rect.y, y2: rect.y + rect.height,
                directions: Meta.BarrierDirection.NEGATIVE_X,
            });
        }
        this._pressure = new Layout.PressureBarrier(PRESSURE, PRESSURE_TIME,
            Shell.ActionMode.NORMAL | Shell.ActionMode.OVERVIEW);
        this._pressure.connect('trigger', () => {
            this.hold('pressure', true);
            this._hovered();
        });
        this._barrier = new Meta.Barrier(params);
        this._pressure.addBarrier(this._barrier);
    }

    _removeBarrier() {
        if (this._pressure && this._barrier)
            this._pressure.removeBarrier(this._barrier);
        this._pressure?.destroy();
        this._pressure = null;
        this._barrier?.destroy();
        this._barrier = null;
    }
}
