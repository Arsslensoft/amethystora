// The drop-down terminal and the scratchpad. One key (Super+Alt+Return) slides a terminal down from the top of the
// screen over whatever is open, on any workspace, and slides it away again, still running: kitty, started once by the
// key and kept. In the classic layout any window can join it from its menu (Move to the scratchpad): the key then shows
// and hides the terminal and those windows together, each where it was left. It is the tiling's floating layer,
// brought to the classic layout; in the tiling the key shows the terminal alone, and the tiling's own key its layer.
//
// Hidden is minimized, with GNOME's effect and the motion module's skipped (the slide is the effect). The shell keeps
// each of these windows on every workspace and above the rest, and gives a window back as it found it when it leaves.
// Nothing is connected while nothing was started or sent there.

import Clutter from 'gi://Clutter';
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import Meta from 'gi://Meta';
import Shell from 'gi://Shell';
import St from 'gi://St';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import * as PopupMenu from 'resource:///org/gnome/shell/ui/popupMenu.js';
import {gettext as _} from 'resource:///org/gnome/shell/extensions/extension.js';

import {reduced, animationsOn} from './util.js';

// The terminal's app id, which kitty takes from --class, and how long after the start a window with it is the one
export const DROP_DOWN_CLASS = 'org.amethystora.DropDown';
const ADOPT = 10 * GLib.USEC_PER_SEC;
// The board's terminal: 14 px in from each side of the screen, half its usable height
const INSET = 14;
const SHARE = 0.5;
const SLIDE = 250;
const FADE = 180;

// Across a screen lock the extension stops and starts again: what it held waits here, by the windows' ids
let waiting = null;

const scale = () => St.ThemeContext.get_for_stage(global.stage).scale_factor;
const windowById = id => global.get_window_actors().map(a => a.meta_window).find(w => w?.get_id() === id) ?? null;

export class Scratch {
    constructor(module) {
        this._module = module;
        this._ext = module.ext;
    }

    enable() {
        this._terminal = null;
        // Each scratchpad window and how it was before it went there
        this._pad = new Map();
        this._launch = null;
        this._waiting = new Set();
        Main.wm.addKeybinding('drop-down-key', this._module.settings, Meta.KeyBindingFlags.IGNORE_AUTOREPEAT,
            Shell.ActionMode.NORMAL, () => this.toggle());
        if (waiting) {
            const {terminal, pad} = waiting;
            waiting = null;
            const window = terminal ? windowById(terminal) : null;
            if (window)
                this._adopt(window, false);
            for (const {id, was} of pad) {
                const each = windowById(id);
                if (each)
                    this._join(each, was);
            }
        }
        this.sync();
    }

    disable() {
        Main.wm.removeKeybinding('drop-down-key');
        this._endLaunch();
        for (const owner of this._waiting ?? [])
            owner.actor.disconnectObject(owner);
        this._waiting = null;
        // A screen lock leaves the windows as they are, to be taken up again at unlock; anything else gives them back
        if (Main.sessionMode.isLocked) {
            if (this._terminal || this._pad?.size) {
                waiting = {
                    terminal: this._terminal?.get_id() ?? null,
                    pad: [...this._pad].map(([w, was]) => ({id: w.get_id(), was})),
                };
            }
            this._terminal?.disconnectObject(this);
            for (const window of this._pad?.keys() ?? [])
                window.disconnectObject(this);
        } else {
            for (const window of [...this._pad?.keys() ?? []])
                this._leave(window, true);
            if (this._terminal)
                this._release(this._terminal, true);
        }
        this._pad = null;
        this._terminal = null;
    }

    // The layout or a switch changed: what is not wanted any more is given back
    sync() {
        if (!this._pad)
            return;
        if (!this._module.settings.get_boolean('drop-down') && this._terminal) {
            this._release(this._terminal, true);
            this._terminal = null;
        }
        if (!this.padOn) {
            for (const window of [...this._pad.keys()])
                this._leave(window, true);
        }
    }

    // The scratchpad is the classic layout's
    get padOn() {
        return this._module.classic && this._module.settings.get_boolean('scratchpad');
    }

    get terminal() {
        return this._terminal ?? null;
    }

    get pad() {
        return [...this._pad?.keys() ?? []];
    }

    holds(window) {
        return !!window && (window === this._terminal || !!this._pad?.has(window));
    }

    // A window that is the terminal the shell started, or on its way to being it
    claims(window) {
        return !!window && (window === this._terminal || this._isLaunched(window));
    }

    // --- the key ------------------------------------------------------------------------------------------------

    toggle() {
        const t0 = GLib.get_monotonic_time();
        const members = this._members();
        const shown = members.filter(w => !w.minimized);
        const focus = global.display.focus_window;
        if (shown.length && members.includes(focus)) {
            for (const window of shown)
                this._hide(window);
        } else if (shown.length) {
            // Shown, but something else has the keyboard: the terminal (or the scratchpad's window used last) takes it
            Main.activateWindow(this._terminal && !this._terminal.minimized ? this._terminal : shown[0]);
        } else {
            for (const window of members)
                this._show(window);
            const first = this._terminal ?? members.at(-1);
            if (first)
                Main.activateWindow(first);
            if (!this._terminal && this._module.settings.get_boolean('drop-down'))
                this._start();
        }
        this._module.note('scratch.toggle', t0);
    }

    // The terminal first, then the scratchpad's windows, used last first
    _members() {
        const list = [];
        if (this._terminal && this._module.settings.get_boolean('drop-down'))
            list.push(this._terminal);
        if (this.padOn) {
            const pad = [...this._pad.keys()];
            pad.sort((a, b) => b.get_user_time() - a.get_user_time());
            list.push(...pad);
        }
        return list;
    }

    // --- the terminal -------------------------------------------------------------------------------------------

    _start() {
        if (this._launch)
            return;
        const argv = ['kitty', '--class', DROP_DOWN_CLASS, '-o', 'hide_window_decorations=yes'];
        let process;
        try {
            const launcher = new Gio.SubprocessLauncher({flags: Gio.SubprocessFlags.NONE});
            // Started from the account's own folder, as a terminal opened from the dock is
            launcher.set_cwd(GLib.get_home_dir());
            process = launcher.spawnv(argv);
        } catch (e) {
            console.warn(`Amethystora UI: the drop-down terminal did not start: ${e.message}`);
            Main.notify(_('The drop-down terminal needs kitty'),
                _('kitty, the image\'s terminal, could not be started.'));
            return;
        }
        const pid = Number(process.get_identifier());
        this._launch = {pid, at: GLib.get_monotonic_time(), process};
        process.wait_async(null, () => {
            if (this._launch?.process === process)
                this._endLaunch();
        });
        global.display.connectObject('window-created', (d, window) => this._created(window), this);
        this._launchTimer = GLib.timeout_add_seconds(GLib.PRIORITY_DEFAULT, ADOPT / GLib.USEC_PER_SEC, () => {
            this._launchTimer = 0;
            this._endLaunch();
            return GLib.SOURCE_REMOVE;
        });
    }

    _endLaunch() {
        if (this._launchTimer)
            GLib.source_remove(this._launchTimer);
        this._launchTimer = 0;
        this._launch = null;
        global.display.disconnectObject(this);
    }

    _isLaunched(window) {
        const launch = this._launch;
        if (!launch || window.get_window_type() !== Meta.WindowType.NORMAL)
            return false;
        if (launch.pid && window.get_pid() === launch.pid)
            return true;
        return window.get_wm_class() === DROP_DOWN_CLASS && GLib.get_monotonic_time() - launch.at < ADOPT;
    }

    _created(window) {
        if (!this._isLaunched(window) || this._terminal)
            return;
        this._endLaunch();
        this._adopt(window, true);
    }

    // The terminal's window, taken in: on every workspace, above, at the top of the screen with the pointer
    _adopt(window, fresh) {
        this._terminal = window;
        window.stick();
        window.make_above();
        window.connectObject('unmanaging', () => {
            window.disconnectObject(this);
            if (this._terminal === window)
                this._terminal = null;
        }, this);
        this._module.heldChanged(window);
        if (!fresh)
            return;
        const actor = window.get_compositor_private();
        if (actor)
            Main.wm.skipNextEffect(actor);
        // Told its place before it draws (Wayland), and put there when it shows whatever it took of it
        const place = this._place();
        const owner = {};
        window.connectObject('configure', (w, config) => {
            if (!config.get_is_initial?.())
                return;
            config.set_position(place.x, place.y);
            config.set_size(place.width, place.height);
        }, 'shown', () => {
            window.disconnectObject(owner);
            const f = window.get_frame_rect();
            if (f.x !== place.x || f.y !== place.y || f.width !== place.width || f.height !== place.height)
                window.move_resize_frame(true, place.x, place.y, place.width, place.height);
            this._slideIn(window);
            // Asked for by the key, so it takes the keyboard whatever Mutter thinks of a window it did not see asked for
            Main.activateWindow(window);
        }, 'unmanaging', () => window.disconnectObject(owner), owner);
    }

    // Where the terminal goes: the work area of the screen with the pointer
    _place() {
        const monitor = global.display.get_current_monitor();
        const area = Main.layoutManager.getWorkAreaForMonitor(monitor);
        const inset = Math.round(INSET * scale());
        return {
            x: area.x + inset, y: area.y,
            width: Math.max(1, area.width - 2 * inset), height: Math.max(1, Math.round(area.height * SHARE)),
        };
    }

    // The terminal given back as an ordinary kitty window
    _release(window, show) {
        window.disconnectObject(this);
        const actor = window.get_compositor_private();
        actor?.remove_all_transitions();
        actor?.set_translation(0, 0, 0);
        if (actor)
            actor.opacity = 255;
        window.unmake_above();
        window.unstick();
        if (show && window.minimized)
            window.unminimize();
        this._module.heldChanged(window);
    }

    // --- the scratchpad -----------------------------------------------------------------------------------------

    // The window menu's line, in the classic layout
    menuItems(menu, window) {
        if (!this.padOn || window === this._terminal || window.get_window_type() !== Meta.WindowType.NORMAL ||
            window.get_transient_for() || window.is_fullscreen())
            return null;
        if (this._pad.has(window)) {
            const item = menu.addAction(_('Take out of the scratchpad'), () => this._leave(window, true));
            item.setOrnament(PopupMenu.Ornament.CHECK);
            return item;
        }
        return menu.addAction(_('Move to the scratchpad'), () => this.send(window));
    }

    // A window joins the scratchpad and goes out of sight with it
    send(window) {
        if (!this.padOn || !window || this.holds(window) || window.get_window_type() !== Meta.WindowType.NORMAL)
            return false;
        this._join(window, {
            sticky: window.is_on_all_workspaces(),
            above: window.is_above(),
            workspace: window.get_workspace()?.index() ?? -1,
        });
        this._hide(window);
        return true;
    }

    _join(window, was) {
        this._pad.set(window, was);
        window.stick();
        window.make_above();
        window.connectObject('unmanaging', () => {
            window.disconnectObject(this);
            this._pad?.delete(window);
        }, this);
        this._module.heldChanged(window);
    }

    // A window leaves the scratchpad as it was before, on the workspace it came from (or the one shown)
    _leave(window, show) {
        const was = this._pad?.get(window);
        if (!was)
            return;
        this._pad.delete(window);
        window.disconnectObject(this);
        const actor = window.get_compositor_private();
        actor?.remove_all_transitions();
        if (actor) {
            actor.opacity = 255;
            actor.set_translation(0, 0, 0);
        }
        if (!was.above)
            window.unmake_above();
        if (!was.sticky) {
            window.unstick();
            const manager = global.workspace_manager;
            const workspace = show ? manager.get_active_workspace()
                : was.workspace >= 0 && was.workspace < manager.n_workspaces
                    ? manager.get_workspace_by_index(was.workspace) : null;
            if (workspace)
                window.change_workspace(workspace);
        }
        if (show && window.minimized) {
            window.unminimize();
            Main.activateWindow(window);
        }
        this._module.heldChanged(window);
    }

    // --- showing and hiding ---------------------------------------------------------------------------------------

    _show(window) {
        const actor = window.get_compositor_private();
        if (window === this._terminal) {
            // On the screen with the pointer each time it comes down
            const place = this._place();
            const f = window.get_frame_rect();
            if (f.x !== place.x || f.y !== place.y || f.width !== place.width || f.height !== place.height)
                window.move_resize_frame(true, place.x, place.y, place.width, place.height);
        }
        if (actor)
            Main.wm.skipNextEffect(actor);
        if (window.minimized)
            window.unminimize();
        if (window === this._terminal)
            this._slideIn(window);
        else
            this._fadeIn(window);
    }

    _hide(window) {
        const actor = window.get_compositor_private();
        const done = () => {
            if (!this.holds(window))
                return;
            const now = window.get_compositor_private();
            if (now)
                Main.wm.skipNextEffect(now);
            window.minimize();
            // Drawn whole again once out of sight, for whatever shows it next
            if (now) {
                now.set_translation(0, 0, 0);
                now.opacity = 255;
            }
        };
        if (!actor || !animationsOn()) {
            done();
            return;
        }
        actor.remove_all_transitions();
        if (window === this._terminal && !reduced(this._ext)) {
            const f = window.get_frame_rect();
            const area = Main.layoutManager.getWorkAreaForMonitor(window.get_monitor());
            actor.ease({
                translation_y: -(f.y - area.y + f.height + INSET * scale()), duration: SLIDE,
                mode: Clutter.AnimationMode.EASE_IN_CUBIC, onStopped: finished => finished && done(),
            });
        } else {
            actor.ease({opacity: 0, duration: FADE, mode: Clutter.AnimationMode.EASE, onStopped: finished => finished && done()});
        }
    }

    _slideIn(window) {
        const actor = window.get_compositor_private();
        if (!actor)
            return;
        actor.remove_all_transitions();
        if (!animationsOn()) {
            actor.set_translation(0, 0, 0);
            actor.opacity = 255;
            return;
        }
        if (reduced(this._ext)) {
            this._fadeIn(window);
            return;
        }
        const f = window.get_frame_rect();
        const area = Main.layoutManager.getWorkAreaForMonitor(window.get_monitor());
        actor.opacity = 255;
        actor.translation_y = -(f.y - area.y + f.height + INSET * scale());
        this._whenMapped(actor, () =>
            actor.ease({translation_y: 0, duration: SLIDE, mode: Clutter.AnimationMode.EASE_OUT_CUBIC}));
    }

    // An actor not yet drawn would end an easing at once: it starts once the window is on the screen again
    _whenMapped(actor, start) {
        if (actor.mapped) {
            start();
            return;
        }
        // However it ends, nothing is left waiting: the part's disable lets go of what still waits
        const owner = {actor};
        actor.connectObject('notify::mapped', () => {
            if (!actor.mapped)
                return;
            actor.disconnectObject(owner);
            this._waiting?.delete(owner);
            start();
        }, 'destroy', () => this._waiting?.delete(owner), owner);
        this._waiting.add(owner);
    }

    _fadeIn(window) {
        const actor = window.get_compositor_private();
        if (!actor)
            return;
        actor.remove_all_transitions();
        actor.set_translation(0, 0, 0);
        if (!animationsOn()) {
            actor.opacity = 255;
            return;
        }
        actor.opacity = 0;
        this._whenMapped(actor, () => actor.ease({opacity: 255, duration: FADE, mode: Clutter.AnimationMode.EASE}));
    }
}
