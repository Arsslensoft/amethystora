// Show the desktop, in the classic layout: every window slides to the nearest side or the bottom of its screen, leaving
// a sliver of each to find it by, and a click anywhere, or any key, brings them back. A click on empty wallpaper starts
// it when show-desktop is on; a hot corner and the three-finger spread start it whatever that says. Only the windows'
// pictures move, so a transparent layer over the screen takes the clicks meanwhile: no click or key ever reaches a
// window nobody can see, and the layer goes the moment anything else happens.

import Atk from 'gi://Atk';
import Clutter from 'gi://Clutter';
import GLib from 'gi://GLib';
import Meta from 'gi://Meta';
import St from 'gi://St';

import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import {gettext as _} from 'resource:///org/gnome/shell/extensions/extension.js';

import {GLIDE, ease, glide, label, reduced} from './util.js';

// The board's sliver and motion
const SLIVER = 28;
const DURATION = 420;
const HINT_TOP = 18;
const TYPES = [Meta.WindowType.NORMAL, Meta.WindowType.DIALOG];

const scale = () => St.ThemeContext.get_for_stage(global.stage).scale_factor;

export class Desktop {
    constructor(module) {
        this._module = module;
        this._ext = module.ext;
        this._out = [];
        this._layers = [];
        this._gestures = [];
        this._progress = 0;
    }

    enable() {
        this._running = true;
        this.syncClick();
    }

    disable() {
        this._running = false;
        this._dropClick();
        this.bringBack(true);
        this._hint?.destroy();
        this._hint = null;
        this._module.surfacesChanged();
    }

    get out() {
        return this._out.length > 0 && this._target === 1;
    }

    get layerCount() {
        return this._layers.length;
    }

    glassSurfaces() {
        return this._hint ? [{actor: this._hint, material: 'glass', radius: 999}] : [];
    }

    // For tests: where each window that is out is, and by how much it moved
    windows() {
        return this._out.map(({window, actor}) => ({
            title: window.get_title(), frame: window.get_frame_rect(), tx: actor.translation_x, ty: actor.translation_y,
        }));
    }

    // --- the wallpaper click --------------------------------------------------------------------------------------

    // A click on the wallpaper, beside GNOME's own long press and right click on it, while show-desktop is on
    syncClick() {
        this._dropClick();
        if (!this._running || !this._module.settings.get_boolean('show-desktop'))
            return;
        this._scan();
        Main.layoutManager.connectObject('monitors-changed', () => {
            // GNOME makes its backgrounds again first
            this._rescanId ||= GLib.idle_add(GLib.PRIORITY_DEFAULT_IDLE, () => {
                this._rescanId = 0;
                this._scan();
                return GLib.SOURCE_REMOVE;
            });
        }, this._clickOwner = {});
    }

    _scan() {
        for (const {actor, gesture} of this._gestures)
            actor.remove_action(gesture);
        this._gestures = [];
        for (const manager of Main.layoutManager._bgManagers ?? []) {
            if (!this._bgOwners?.has(manager)) {
                (this._bgOwners ??= new Set()).add(manager);
                manager.connectObject('changed', () => {
                    this._rescanId ||= GLib.idle_add(GLib.PRIORITY_DEFAULT_IDLE, () => {
                        this._rescanId = 0;
                        this._scan();
                        return GLib.SOURCE_REMOVE;
                    });
                }, this._clickOwner);
            }
            const actor = manager.backgroundActor;
            if (!actor)
                continue;
            const gesture = new Clutter.ClickGesture({required_button: Clutter.BUTTON_PRIMARY});
            gesture.connect('recognize', () => {
                if (!Main.overview.visible && this._module.classic)
                    this.toggle();
            });
            actor.add_action(gesture);
            this._gestures.push({actor, gesture});
        }
    }

    _dropClick() {
        if (this._rescanId)
            GLib.source_remove(this._rescanId);
        this._rescanId = 0;
        for (const {actor, gesture} of this._gestures)
            actor.remove_action(gesture);
        this._gestures = [];
        if (this._clickOwner) {
            Main.layoutManager.disconnectObject(this._clickOwner);
            for (const manager of this._bgOwners ?? [])
                manager.disconnectObject(this._clickOwner);
        }
        this._bgOwners = null;
        this._clickOwner = null;
    }

    // --- out and back ---------------------------------------------------------------------------------------------

    toggle() {
        if (this._out.length && this._target === 1)
            this.bringBack();
        else
            this.showOut();
        return this.out;
    }

    // Which edge a window goes to: the nearest of left, right and bottom that no other screen shares; when all three
    // are shared, the bottom
    _edge(window, area, monitor) {
        const f = window.get_frame_rect();
        const cx = f.x + f.width / 2;
        const cy = f.y + f.height / 2;
        const others = Main.layoutManager.monitors.filter(m => m.index !== monitor.index);
        const overlapsY = m => m.y < monitor.y + monitor.height && m.y + m.height > monitor.y;
        const overlapsX = m => m.x < monitor.x + monitor.width && m.x + m.width > monitor.x;
        const shared = {
            left: others.some(m => m.x + m.width === monitor.x && overlapsY(m)),
            right: others.some(m => m.x === monitor.x + monitor.width && overlapsY(m)),
            bottom: others.some(m => m.y === monitor.y + monitor.height && overlapsX(m)),
        };
        const distances = [
            ['left', cx - area.x], ['right', area.x + area.width - cx], ['bottom', area.y + area.height - cy],
        ].filter(([edge]) => !shared[edge]).sort((a, b) => a[1] - b[1]);
        const edge = distances[0]?.[0] ?? 'bottom';
        const sliver = SLIVER * scale();
        if (edge === 'left')
            return [area.x + sliver - (f.x + f.width), 0];
        if (edge === 'right')
            return [area.x + area.width - sliver - f.x, 0];
        return [0, area.y + area.height - sliver - f.y];
    }

    _collect() {
        const workspace = global.workspace_manager.get_active_workspace();
        return global.get_window_actors().filter(actor => {
            const window = actor.meta_window;
            return window && TYPES.includes(window.get_window_type()) && !window.minimized &&
                window.located_on_workspace(workspace) && !this._module.isKept(window);
        }).map(actor => {
            const window = actor.meta_window;
            const monitor = Main.layoutManager.monitors[window.get_monitor()];
            const area = Main.layoutManager.getWorkAreaForMonitor(window.get_monitor());
            const [tx, ty] = this._edge(window, area, monitor);
            return {window, actor, tx, ty};
        });
    }

    // Starts the windows on their way out; with a gesture, begin() and then progress() follow the fingers
    // The windows to send out, and the layer that takes the clicks meanwhile. Windows still on their way back
    // (bringBack takes the layer away at once and lets them glide) go out again from where they are, and the layer
    // comes back with them: windows are never out without it.
    _prepare() {
        if (!this._module.classic)
            return false;
        const fresh = this._collect();
        if (!this._out.length) {
            this._out = fresh;
        } else {
            this._timeline?.stop();
            this._timeline = null;
            const known = new Set(this._out.map(o => o.window));
            this._out.push(...fresh.filter(o => !known.has(o.window)));
        }
        if (!this._out.length)
            return false;
        if (!this._watch)
            this._arm();
        return true;
    }

    showOut() {
        if (!this._prepare())
            return false;
        this._animateTo(1);
        return true;
    }

    begin() {
        if (this._out.length && this._target === 1 && this._watch)
            return false;
        if (!this._prepare())
            return false;
        this._target = 1;
        return true;
    }

    // Follows a gesture: 0 is every window in place, 1 every window out
    progress(t) {
        if (!this._out.length)
            return;
        this._set(Math.max(0, Math.min(1, t)));
    }

    // Ends a gesture: out if it went far enough, else back
    end(finish) {
        if (!this._out.length)
            return;
        if (finish)
            this._animateTo(1);
        else
            this.bringBack();
    }

    _set(p) {
        const t0 = GLib.get_monotonic_time();
        this._progress = p;
        for (const {window, actor, tx, ty} of this._out) {
            // A window closed on its way back has no actor to move any more
            if (window.get_compositor_private() !== actor)
                continue;
            actor.translation_x = tx * p;
            actor.translation_y = ty * p;
        }
        this._module.note('desktopFrame', t0);
    }

    _animateTo(target) {
        this._target = target;
        this._timeline?.stop();
        this._timeline = null;
        const from = this._progress;
        this._syncHint(target === 1);
        const done = () => {
            this._timeline = null;
            if (target === 0)
                this._disarm();
        };
        if (reduced(this._ext)) {
            // A crossfade in place of the slide: the windows fade, jump, and come back in
            const actors = this._out.map(o => o.actor);
            if (!actors.length) {
                done();
                return;
            }
            actors.forEach((actor, i) => actor.ease({
                opacity: 0, duration: 75, mode: Clutter.AnimationMode.EASE,
                onComplete: () => {
                    if (i === 0)
                        this._set(target);
                    actor.ease({opacity: 255, duration: 75, mode: Clutter.AnimationMode.EASE,
                        onComplete: i === 0 ? done : undefined});
                },
            }));
            return;
        }
        const span = Math.abs(target - from);
        this._timeline = glide(global.stage, DURATION * Math.max(span, 0.05), GLIDE,
            p => this._set(from + (target - from) * p), done);
    }

    // The layer that takes clicks and keys while the windows are out, and what brings them back
    _arm() {
        this._watch = {};
        const back = () => this.bringBack();
        for (const monitor of Main.layoutManager.monitors) {
            const area = Main.layoutManager.getWorkAreaForMonitor(monitor.index);
            const layer = new St.Widget({reactive: true, can_focus: true, name: 'amethystoraWindowsDesktopLayer',
                x: area.x, y: area.y, width: area.width, height: area.height,
                accessible_role: Atk.Role.PUSH_BUTTON, accessible_name: _('Bring the windows back')});
            Main.layoutManager.addChrome(layer);
            const click = new Clutter.ClickGesture();
            click.connect('recognize', () => {
                const {x, y} = click.get_coords_abs();
                this._clicked(x, y);
            });
            layer.add_action(click);
            layer.connect('key-press-event', (actor, event) => {
                // A modifier alone (Shift, Ctrl, Alt, Super, the level shifts) is not yet a key pressed: the windows
                // stay out until the key it goes with
                const symbol = event.get_key_symbol();
                if ((symbol >= Clutter.KEY_Shift_L && symbol <= Clutter.KEY_Hyper_R) ||
                    (symbol >= Clutter.KEY_ISO_Lock && symbol <= Clutter.KEY_ISO_Level5_Lock))
                    return Clutter.EVENT_PROPAGATE;
                back();
                return Clutter.EVENT_STOP;
            });
            this._layers.push(layer);
        }
        this._focused = global.display.focus_window;
        const primary = this._layers[Main.layoutManager.primaryIndex] ?? this._layers[0];
        global.stage.set_key_focus(primary);

        global.workspace_manager.connectObject('active-workspace-changed', back, this._watch);
        Main.overview.connectObject('showing', back, this._watch);
        global.display.connectObject('window-created', back, 'grab-op-begin', back,
            'notify::focus-window', () => {
                if (global.display.focus_window)
                    back();
            }, this._watch);
        Main.layoutManager.connectObject('monitors-changed', back, this._watch);
        for (const {window} of this._out) {
            window.connectObject('notify::minimized', back,
                'unmanaging', () => this._drop(window), this._watch);
        }
    }

    _drop(window) {
        const index = this._out.findIndex(o => o.window === window);
        if (index >= 0) {
            window.disconnectObject(this._watch);
            this._out.splice(index, 1);
        }
        if (!this._out.length)
            this._disarm();
    }

    _clicked(x, y) {
        // A click on a window's sliver brings that window to the front too
        const hit = this._out.find(({window, actor}) => {
            const f = window.get_frame_rect();
            const left = f.x + actor.translation_x;
            const top = f.y + actor.translation_y;
            return x >= left && x < left + f.width && y >= top && y < top + f.height;
        });
        this.bringBack();
        if (hit)
            Main.activateWindow(hit.window);
    }

    // Everything back where it was; at once when the module stops
    bringBack(now = false) {
        if (!this._out.length)
            return;
        if (now) {
            this._timeline?.stop();
            this._timeline = null;
            this._set(0);
            this._disarm();
            return;
        }
        this._removeLayers();
        this._animateTo(0);
    }

    _removeLayers() {
        const focused = this._layers.some(l => global.stage.get_key_focus() === l);
        for (const layer of this._layers)
            layer.destroy();
        this._layers = [];
        if (focused) {
            global.stage.set_key_focus(null);
            // The keyboard back to the window that had it, without raising anything
            const window = this._focused;
            if (window && !window.minimized && window.get_compositor_private())
                window.focus(global.get_current_time());
        }
        this._focused = null;
        if (this._watch) {
            global.workspace_manager.disconnectObject(this._watch);
            Main.overview.disconnectObject(this._watch);
            global.display.disconnectObject(this._watch);
            Main.layoutManager.disconnectObject(this._watch);
            for (const {window} of this._out)
                window.disconnectObject(this._watch);
        }
        this._watch = null;
    }

    _disarm() {
        this._removeLayers();
        for (const {window, actor} of this._out) {
            if (window.get_compositor_private() !== actor)
                continue;
            actor.translation_x = 0;
            actor.translation_y = 0;
        }
        this._out = [];
        this._progress = 0;
        this._target = 0;
        this._syncHint(false);
    }

    // The pill that says how to bring them back, on the main screen
    _syncHint(show) {
        if (show && !this._hint) {
            this._hint = label(_('Click anywhere to bring the windows back'), 'amethystora-windows-desktop-hint');
            this._hint.opacity = 0;
            Main.layoutManager.uiGroup.add_child(this._hint);
            this._module.surfacesChanged();
        }
        if (!this._hint)
            return;
        if (show) {
            const area = Main.layoutManager.getWorkAreaForMonitor(Main.layoutManager.primaryIndex);
            const [, width] = this._hint.get_preferred_width(-1);
            this._hint.set_position(Math.round(area.x + (area.width - width) / 2), area.y + HINT_TOP * scale());
            Main.layoutManager.uiGroup.set_child_above_sibling(this._hint, null);
            this._hint.show();
        }
        const hint = this._hint;
        ease(this._ext, hint, {opacity: show ? 255 : 0}, {
            duration: 160,
            onComplete: () => {
                if (!show && this._target !== 1)
                    hint.hide();
            },
        });
    }
}
