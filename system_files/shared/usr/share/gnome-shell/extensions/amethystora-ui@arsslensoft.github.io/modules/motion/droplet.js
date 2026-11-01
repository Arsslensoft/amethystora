// The drop of glass a window becomes on its way into the dock and back. While it flies, the window's own picture is
// drawn once through a small shader in place of the window: cut to its frame, rounded to the flight's corners, and
// lit by a glossy gradient of the theme's accent, as strong as the flight says. The window's scale and translation
// put the picture where the flight wants it; the shader never moves anything. Nothing is copied: it reads the
// texture the window already has. A copy of the window drawn elsewhere (the overview, a switcher's picture) shows the
// plain window.

import Clutter from 'gi://Clutter';
import GObject from 'gi://GObject';

import {Shader} from '../glass/shader.js';

// The shader every flight draws with, one per module; each flight has its own copy of the pipeline
export function dropletShader() {
    return new Shader(DECLARATIONS, 'cogl_color_out = droplet_color();', 1);
}

const DECLARATIONS = `
uniform sampler2D u_window;
// The frame inside the window's picture, as shares of it (left, top, right, bottom)
uniform vec4 u_frame;
// The drop as drawn now: its size and its corners in stage pixels, and one device pixel of soft edge
uniform vec2 u_size;
uniform float u_radius;
uniform float u_aa;
// How strong the gloss is, the theme's accent, and the opacity of the whole
uniform float u_gloss;
uniform vec3 u_accent;
uniform float u_opacity;
// How much of the window is drawn as itself, its own corners and shadow included: all of it at either end
uniform float u_plain;

vec4 droplet_color() {
    vec2 st = cogl_tex_coord_in[0].st;
    vec2 across = (st - u_frame.xy) / (u_frame.zw - u_frame.xy);
    vec2 p = (across - 0.5) * u_size;
    vec2 q = abs(p) - (0.5 * u_size - vec2(u_radius));
    float dist = length(max(q, 0.0)) + min(max(q.x, q.y), 0.0) - u_radius;
    float inside = clamp(0.5 - dist / u_aa, 0.0, 1.0);
    vec4 picture = texture2D(u_window, st);

    // The highlight sits up and to the left; the colour deepens towards the far corner
    vec2 from_light = (across - vec2(0.34, 0.28)) * u_size;
    float t = clamp(length(from_light) / length(vec2(0.66, 0.72) * u_size), 0.0, 1.0);
    vec4 white = vec4(1.0, 1.0, 1.0, 0.95);
    vec4 pale = vec4(mix(u_accent, vec3(1.0), 0.65), 0.75);
    vec4 tint = vec4(u_accent, 0.85);
    vec4 deep = vec4(u_accent * 0.5, 0.95);
    vec4 gloss;
    if (t < 0.18)
        gloss = mix(white, pale, t / 0.18);
    else if (t < 0.55)
        gloss = mix(pale, tint, (t - 0.18) / 0.37);
    else
        gloss = mix(tint, deep, (t - 0.55) / 0.45);
    float a = gloss.a * u_gloss;
    vec4 lit = vec4(gloss.rgb * a, a) + picture * (1.0 - a);
    return mix(lit * inside, picture, u_plain) * u_opacity;
}
`;

export const DropletEffect = GObject.registerClass(
class AmethystoraMotionDroplet extends Clutter.Effect {
    _init(shader, accent) {
        super._init();
        this._shader = shader;
        this._accent = accent;
        // While the drop is over the dock and the top bar, it is drawn by a LiftedCopy, which says when it paints
        this.lifted = false;
        this.copyPainting = false;
        this._pipeline = null;
        this._texture = null;
        // What the flight draws now: {w, h, r, g, o}
        this.state = null;
        // The frame inside the window's picture, as shares of it (left, top, right, bottom)
        this.frame = null;
    }

    // Taken off its window at the end of the flight: the pipeline goes with it
    release() {
        this._pipeline = null;
        this._texture = null;
    }

    vfunc_modify_paint_volume(_volume) {
        return true;
    }

    vfunc_paint_node(node, paintContext, _flags) {
        const actor = this.get_actor();
        const state = this.state;
        const copy = actor.is_in_clone_paint();
        // Lifted over the dock, the drop is drawn by its own copy up there, and not in the windows' layer
        if (this.lifted && !copy)
            return;
        const ours = copy ? this.lifted && this.copyPainting : true;
        const texture = state && ours ? this._windowTexture(actor) : null;
        if (!texture) {
            node.add_child(new Clutter.ActorNode(actor, -1));
            return;
        }
        if (!this._pipeline) {
            this._pipeline = this._shader.pipeline(paintContext.get_framebuffer().get_context());
            this._shader.setInt(this._pipeline, 'u_window', 0);
        }
        const pipeline = this._pipeline;
        if (this._texture !== texture) {
            pipeline.set_layer_texture(0, texture);
            this._texture = texture;
        }
        this._shader.set(pipeline, 'u_frame', ...this.frame ?? [0, 0, 1, 1]);
        this._shader.set(pipeline, 'u_size', state.w, state.h);
        this._shader.set(pipeline, 'u_radius', state.r);
        this._shader.set(pipeline, 'u_aa', 1 / Math.max(1, actor.get_resource_scale()));
        this._shader.set(pipeline, 'u_gloss', state.g);
        this._shader.set(pipeline, 'u_accent', ...this._accent);
        this._shader.set(pipeline, 'u_opacity', state.o * actor.get_paint_opacity() / 255);
        this._shader.set(pipeline, 'u_plain', state.plain ?? 0);
        const quad = new Clutter.PipelineNode(pipeline);
        node.add_child(quad);
        quad.add_texture_rectangle(new Clutter.ActorBox({x1: 0, y1: 0, x2: actor.width, y2: actor.height}),
            0, 0, 1, 1);
    }

    // The window's own picture, when it is one plain texture (a video's YUV buffer is not: that window flies without
    // its rounding)
    _windowTexture(actor) {
        const multi = actor.get_texture?.()?.get_texture?.();
        if (!multi || !multi.is_simple())
            return null;
        return multi.get_plane(0);
    }
});

// The drop while it is near the dock: drawn above the dock and the top bar, where the windows' layer cannot reach. It
// is a copy only in the sense of a second place to paint the same window from, never a picture of it: it holds no
// texture, and the window's own effect draws the drop through it.
export const LiftedCopy = GObject.registerClass(
class AmethystoraMotionLifted extends Clutter.Clone {
    _init(effect, source) {
        super._init({source, reactive: false});
        this._effect = effect;
    }

    vfunc_paint(paintContext) {
        const effect = this._effect;
        if (effect)
            effect.copyPainting = true;
        try {
            super.vfunc_paint(paintContext);
        } finally {
            if (effect)
                effect.copyPainting = false;
        }
    }
});
