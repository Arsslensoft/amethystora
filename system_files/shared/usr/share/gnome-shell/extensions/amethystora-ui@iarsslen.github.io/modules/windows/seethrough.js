// See-through windows (offered, off): Super+Alt and the scroll wheel over a window set how much of what is behind it
// shows through, from 40 % to fully solid, in steps of 5 %; the window forgets it when it closes. A rule can keep an
// app's windows so (the terminal at 92 %, say). A window the share guard covers is always drawn solid.
//
// Mutter hands a scroll made with the window-drag modifier held to the shell rather than to the app (its events.c),
// and GNOME switches workspaces with it as it bubbles up to the stage; this takes the scroll first, on its way down,
// only with Super and Alt both held and only over a window, and leaves every other scroll to GNOME. The opacity is that
// of the window's own surfaces inside its actor: GNOME's and the motion module's fades of the actor itself stay theirs,
// and nothing else is drawn. Connected only while the key is on.

import Clutter from 'gi://Clutter';
import GLib from 'gi://GLib';
import Meta from 'gi://Meta';
import Shell from 'gi://Shell';
import St from 'gi://St';

import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import {gettext as _} from 'resource:///org/gnome/shell/extensions/extension.js';

import {appOf} from './places.js';
import {Osd} from './osd.js';
import {caps} from './util.js';

export const MIN = 40;
const MAX = 100;
const STEP = 5;
// How far in from the window's corner the percentage shows (the board's)
const INSET = 18;
const TYPES = [Meta.WindowType.NORMAL, Meta.WindowType.DIALOG, Meta.WindowType.MODAL_DIALOG, Meta.WindowType.UTILITY];
// Mutter's surface classes are not in its introspection data: they are told by their type's name
const SURFACE = /Surface(Container)?Actor/;

const SUPER = Clutter.ModifierType.MOD4_MASK | Clutter.ModifierType.SUPER_MASK;
const ALT = Clutter.ModifierType.MOD1_MASK;

// The window whose picture an actor is part of
function windowOf(actor) {
    for (let a = actor; a; a = a.get_parent()) {
        if (a instanceof Meta.WindowActor)
            return a.meta_window;
    }
    return null;
}

export class SeeThrough {
    constructor(module) {
        this._module = module;
        this._ext = module.ext;
    }

    enable() {
        // The windows drawn see-through, and how much (a percentage below 100)
        this._levels = new Map();
        this._smooth = 0;
        this._osd = new Osd();
        global.stage.connectObject('captured-event::scroll', (stage, event) => this._scroll(event), this);
        this._module.settings.connectObject('changed::see-through-apps', () => this._rulesChanged(), this);
        this._rulesChanged();
    }

    disable() {
        global.stage.disconnectObject(this);
        global.display.disconnectObject(this);
        this._module.settings?.disconnectObject(this);
        for (const window of [...this._levels?.keys() ?? []])
            this._set(window, MAX);
        this._levels = null;
        this._hidePill();
    }

    // For the other parts and the tests: a window's percentage
    level(window) {
        return this._levels?.get(window) ?? MAX;
    }

    // The share guard's covers changed: a covered window is solid, an uncovered one has its own level back
    coversChanged() {
        for (const [window, level] of this._levels ?? [])
            this._apply(window, level);
    }

    // --- the rules ----------------------------------------------------------------------------------------------

    _rules() {
        try {
            const rules = this._module.settings.get_value('see-through-apps').deep_unpack();
            return rules && typeof rules === 'object' ? rules : {};
        } catch {
            return {};
        }
    }

    _ruleFor(window) {
        const id = appOf(window);
        const level = id ? this._rules()[id] : undefined;
        return Number.isInteger(level) ? Math.max(MIN, Math.min(MAX, level)) : null;
    }

    // New windows are watched only while some app has a rule; the windows open now take their app's rule at once
    _rulesChanged() {
        const any = Object.keys(this._rules()).length > 0;
        global.display.disconnectObject(this);
        if (any) {
            global.display.connectObject('window-created', (d, window) => {
                if (!TYPES.includes(window.get_window_type()))
                    return;
                const owner = {};
                window.connectObject('shown', () => {
                    window.disconnectObject(owner);
                    const level = this._ruleFor(window);
                    if (level !== null && this._levels)
                        this._set(window, level);
                }, 'unmanaging', () => window.disconnectObject(owner), owner);
            }, this);
        }
        for (const actor of global.get_window_actors()) {
            const window = actor.meta_window;
            if (!window || !TYPES.includes(window.get_window_type()))
                continue;
            const level = this._ruleFor(window);
            if (level !== null)
                this._set(window, level);
        }
    }

    // --- the scroll ---------------------------------------------------------------------------------------------

    _scroll(event) {
        const state = event.get_state();
        if (!(state & SUPER) || !(state & ALT) || Main.actionMode !== Shell.ActionMode.NORMAL)
            return Clutter.EVENT_PROPAGATE;
        const window = this._windowAt(event);
        if (!window)
            return Clutter.EVENT_PROPAGATE;
        // One turn of a wheel comes as a step and as a smooth move made from it (or the other way round): the copy
        // is taken away from GNOME too, and counts for nothing
        if (event.get_flags() & Clutter.EventFlags.FLAG_POINTER_EMULATED)
            return Clutter.EVENT_STOP;
        let steps = 0;
        switch (event.get_scroll_direction()) {
        case Clutter.ScrollDirection.UP:
            steps = 1;
            break;
        case Clutter.ScrollDirection.DOWN:
            steps = -1;
            break;
        case Clutter.ScrollDirection.SMOOTH: {
            // A touchpad's or a fine wheel's small moves add up to whole steps
            const [, dy] = event.get_scroll_delta();
            this._smooth -= dy;
            steps = Math.trunc(this._smooth);
            this._smooth -= steps;
            break;
        }
        default:
            return Clutter.EVENT_STOP;
        }
        if (steps)
            this.step(window, steps);
        return Clutter.EVENT_STOP;
    }

    _windowAt(event) {
        let window = windowOf(global.stage.get_event_actor(event));
        if (!window) {
            // Under the pointer, the window on top there
            const [x, y] = event.get_coords();
            const workspace = global.workspace_manager.get_active_workspace();
            const windows = global.display.sort_windows_by_stacking(workspace.list_windows()
                .filter(w => w.showing_on_its_workspace?.() ?? !w.minimized));
            window = windows.reverse().find(w => {
                const f = w.get_frame_rect();
                return x >= f.x && x < f.x + f.width && y >= f.y && y < f.y + f.height;
            }) ?? null;
        }
        if (!window || !TYPES.includes(window.get_window_type()) || window.is_override_redirect())
            return null;
        return window;
    }

    // A window a number of steps more solid (or, below 0, less); held at the ends
    step(window, steps) {
        const t0 = GLib.get_monotonic_time();
        if (this._covered(window))
            return this.level(window);
        const level = Math.max(MIN, Math.min(MAX, this.level(window) + steps * STEP));
        this._set(window, level);
        this._showPill(window, level);
        this._module.note('seeThrough.step', t0);
        return level;
    }

    // --- the window's picture -------------------------------------------------------------------------------------

    _covered(window) {
        return this._ext.modules?.get('privacy')?.covers?.(window) ?? false;
    }

    _set(window, level) {
        if (!this._levels)
            return;
        const had = this._levels.has(window);
        if (level >= MAX) {
            this._levels.delete(window);
            if (had) {
                window.disconnectObject(this);
                window.get_compositor_private()?.disconnectObject(this);
            }
        } else {
            this._levels.set(window, level);
            if (!had) {
                window.connectObject('unmanaging', () => {
                    window.disconnectObject(this);
                    this._levels?.delete(window);
                }, this);
                // A surface Mutter makes again takes the window's level
                window.get_compositor_private()?.connectObject('child-added', () =>
                    this._apply(window, this.level(window)), this);
            }
        }
        this._apply(window, level);
    }

    _apply(window, level) {
        const actor = window.get_compositor_private();
        if (!actor)
            return;
        const opacity = this._covered(window) ? 255 : Math.round(255 * level / MAX);
        for (const child of actor.get_children()) {
            if (SURFACE.test(child.constructor?.name ?? ''))
                child.opacity = opacity;
        }
        // A window kept on top keeps its ring and loses its glow while it is see-through
        if (this._module.running('ontop'))
            this._module.ontop.ringOf(window)?.sync();
    }

    // --- the percentage shown -------------------------------------------------------------------------------------

    _showPill(window, level) {
        const s = St.ThemeContext.get_for_stage(global.stage).scale_factor;
        const f = window.get_frame_rect();
        // The percentage in the session's own way of writing numbers, inside the window's bottom corner
        this._osd.show(_('%s %%').format(level.toLocaleString()), {
            keys: caps('<Super><Alt>a')?.slice(0, 2),
            at: [f.x + INSET * s, f.y + f.height - INSET * s],
        });
    }

    _hidePill() {
        this._osd?.hide();
    }

    get pill() {
        return this._osd?.shown ?? null;
    }
}
