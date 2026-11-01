// Dimming the windows not in use (offered, off): every window of the workspace but the one being used, its dialogs and
// the windows it stands in front of, is drawn at two thirds of its brightness, so the eye goes to the right one. No
// copy of a window is made: the effect draws the window, then its own picture once more in translucent black over it,
// which darkens every pixel the window covers and leaves its transparent corners and shadow alone. Clones (the
// overview, the switchers) are not dimmed.

import Clutter from 'gi://Clutter';
import Cogl from 'gi://Cogl';
import GObject from 'gi://GObject';
import Meta from 'gi://Meta';
import St from 'gi://St';

// rgb · (1 − 0.34) is the board's brightness(.66)
const DARKEN = 0.34;
const DURATION = 300;
export const NAME = 'amethystora-dim';
const TYPES = [Meta.WindowType.NORMAL, Meta.WindowType.DIALOG, Meta.WindowType.MODAL_DIALOG];

export const DimEffect = GObject.registerClass({
    Properties: {
        'strength': GObject.ParamSpec.double('strength', null, null, GObject.ParamFlags.READWRITE, 0, 1, 0),
    },
}, class AmethystoraDimEffect extends Clutter.Effect {
    _init() {
        super._init();
        this._strength = 0;
        this._pipeline = null;
        this._context = null;
        this._texture = null;
    }

    get strength() {
        return this._strength;
    }

    set strength(value) {
        if (this._strength === value)
            return;
        this._strength = value;
        this.notify('strength');
        this.get_actor()?.queue_redraw();
    }

    // The actor that shows the window's main surface, inside the window's actor
    _surface(actor, texture) {
        if (this._surfaceActor?.get_parent() && this._surfaceTexture === texture)
            return this._surfaceActor;
        const find = (parent, depth) => {
            for (const child of parent.get_children()) {
                if (child.get_content?.() === texture || child.get_texture?.() === texture)
                    return child;
                const deeper = depth < 3 ? find(child, depth + 1) : null;
                if (deeper)
                    return deeper;
            }
            return null;
        };
        this._surfaceActor = find(actor, 0);
        this._surfaceTexture = texture;
        return this._surfaceActor;
    }

    vfunc_paint_node(node, paintContext, _flags) {
        const actor = this.get_actor();
        node.add_child(new Clutter.ActorNode(actor, -1));
        if (this._strength <= 0 || actor.is_in_clone_paint())
            return;
        const shaped = actor.get_texture?.();
        const multi = shaped?.get_texture?.();
        if (!multi?.is_simple?.())
            return;
        const plane = multi.get_plane(0);
        const surface = this._surface(actor, shaped);
        if (!plane || !surface)
            return;
        const context = paintContext.get_framebuffer().get_context();
        if (this._context !== context) {
            this._pipeline = Cogl.Pipeline.new(context);
            this._pipeline.set_layer_combine(0, 'RGBA = MODULATE (TEXTURE, PRIMARY)');
            this._context = context;
            this._texture = null;
        }
        if (this._texture !== plane) {
            this._pipeline.set_layer_texture(0, plane);
            this._texture = plane;
        }
        // Premultiplied black at the strength's share: the window's alpha takes it where the window is
        const colour = new Cogl.Color();
        // As faint as the window is drawn: a window fading out takes its shade with it
        colour.init_from_4f(0, 0, 0, DARKEN * this._strength * actor.get_paint_opacity() / 255);
        this._pipeline.set_color(colour);
        // Where the surface lies inside the window's actor
        let [x, y] = [0, 0];
        for (let a = surface; a && a !== actor; a = a.get_parent()) {
            const box = a.get_allocation_box();
            x += box.x1;
            y += box.y1;
        }
        const pipelineNode = new Clutter.PipelineNode(this._pipeline);
        node.add_child(pipelineNode);
        pipelineNode.add_texture_rectangle(new Clutter.ActorBox({
            x1: x, y1: y, x2: x + surface.width, y2: y + surface.height,
        }), 0, 0, 1, 1);
    }
});

export class Dim {
    constructor(module) {
        this._module = module;
        this._ext = module.ext;
    }

    enable() {
        this._new = new Set();
        const update = () => this.update();
        // A new window has its actor only once it shows; who covers whom changes on a restack or after a move
        global.display.connectObject('notify::focus-window', update, 'window-created', (d, window) => {
            // Watched until it shows or goes, whichever comes first
            const done = () => {
                window.disconnectObject(this);
                this._new?.delete(window);
            };
            window.connectObject('shown', () => {
                done();
                update();
            }, 'unmanaging', done, this);
            this._new.add(window);
        }, 'restacked', update, 'grab-op-end', update, this);
        global.workspace_manager.connectObject('active-workspace-changed', update, this);
        this.update(true);
    }

    disable() {
        global.display.disconnectObject(this);
        for (const window of this._new ?? [])
            window.disconnectObject(this);
        this._new = null;
        global.workspace_manager.disconnectObject(this);
        for (const actor of global.get_window_actors()) {
            const effect = actor.get_effect(NAME);
            if (effect) {
                actor.remove_transition('@effects.amethystora-dim.strength');
                actor.remove_effect(effect);
            }
        }
    }

    // Whether a window is one the person is not using now
    _dimmed(window, focus, workspace) {
        if (!focus || !TYPES.includes(window.get_window_type()) || window.minimized || window.is_fullscreen() ||
            window.skip_taskbar || !window.located_on_workspace(workspace))
            return false;
        // The focused window, the dialogs over it, and the window a focused dialog belongs to
        for (let w = window; w; w = w.get_transient_for()) {
            if (w === focus)
                return false;
        }
        for (let w = focus; w; w = w.get_transient_for()) {
            if (w === window)
                return false;
        }
        return true;
    }

    // The windows wholly behind one window above them: nobody sees them, and an effect would keep Mutter from
    // leaving them out of the frame. The glass's own list of covered windows counts too.
    _covered(workspace) {
        const covered = new Set(this._ext.modules?.get('glass')?.windows?.covered ?? []);
        const shown = global.display.sort_windows_by_stacking(global.get_window_actors().map(a => a.meta_window)
            .filter(w => w && !w.minimized && w.located_on_workspace(workspace)));
        for (let i = 0; i < shown.length; i++) {
            const f = shown[i].get_frame_rect();
            for (let j = i + 1; j < shown.length; j++) {
                const g = shown[j].get_frame_rect();
                if (g.x <= f.x && g.y <= f.y && g.x + g.width >= f.x + f.width && g.y + g.height >= f.y + f.height) {
                    covered.add(shown[i]);
                    break;
                }
            }
        }
        return covered;
    }

    update(now = false) {
        this._module.count('dimUpdate');
        const focus = global.display.focus_window;
        const workspace = global.workspace_manager.get_active_workspace();
        const covered = this._covered(workspace);
        for (const actor of global.get_window_actors()) {
            const window = actor.meta_window;
            if (!window)
                continue;
            const dim = this._dimmed(window, focus, workspace) && !covered.has(window);
            let effect = actor.get_effect(NAME);
            if (!dim && !effect)
                continue;
            if (!effect) {
                effect = new DimEffect();
                actor.add_effect_with_name(NAME, effect);
            }
            const target = dim ? 1 : 0;
            if (effect.strength === target && !actor.get_transition('@effects.amethystora-dim.strength'))
                continue;
            const done = () => {
                // An undimmed window carries no effect, so that Mutter culls it as before
                if (!dim && actor.get_effect(NAME) === effect)
                    actor.remove_effect(effect);
            };
            if (now || !St.Settings.get().enable_animations) {
                actor.remove_transition('@effects.amethystora-dim.strength');
                effect.strength = target;
                done();
                continue;
            }
            actor.ease_property('@effects.amethystora-dim.strength', target, {
                duration: DURATION, mode: Clutter.AnimationMode.EASE, onComplete: done,
            });
        }
    }

    // For tests: each window's title and strength
    strengths() {
        return global.get_window_actors().map(actor => [actor.meta_window?.get_title(),
            actor.get_effect(NAME)?.strength ?? 0]);
    }
}
