// A capture drawn with round corners: the thumbnail's picture, cut to cover its box, and a pinned picture whole. Clutter
// clips only to rectangles, so the picture is drawn through a small shader that leaves out what lies beyond the corners;
// the texture is the capture's own, made once, and nothing is copied when the actor is moved, scaled or faded.

import Clutter from 'gi://Clutter';
import Cogl from 'gi://Cogl';
import GObject from 'gi://GObject';

import {Shader} from '../glass/shader.js';

// The rounding, in the box's own pixels: t runs over the part of the texture drawn (crop), p is the point in the box,
// d its distance outside the rounded rectangle, and a pixel within half a pixel of the edge is partly drawn
const SHADER = new Shader(`
uniform vec4 crop;
uniform vec2 size;
uniform float radius;
`, `
vec2 t = (cogl_tex_coord_in[0].st - crop.xy) / max(crop.zw - crop.xy, vec2(0.0001));
vec2 p = t * size;
vec2 q = abs(p - size * 0.5) - (size * 0.5 - vec2(radius));
float d = length(max(q, 0.0)) + min(max(q.x, q.y), 0.0) - radius;
cogl_color_out *= clamp(0.5 - d, 0.0, 1.0);
`);

export const RoundedPicture = GObject.registerClass(
class AmethystoraRoundedPicture extends Clutter.Actor {
    // content: an St.ImageContent of the picture; width and height: its size in pixels; radius in logical pixels;
    // cover: cut the picture to fill the box (the thumbnail) rather than stretch it (a pin keeps its proportions itself)
    _init(content, width, height, radius, cover, params = {}) {
        super._init(params);
        this._content = content;
        this._pixels = [width, height];
        this._radius = radius;
        this._cover = cover;
        this._pipeline = null;
        this._context = null;
    }

    set radius(value) {
        this._radius = value;
        this.queue_redraw();
    }

    // The part of the texture drawn, as [u0, v0, u1, v1]
    _crop(width, height) {
        if (!this._cover || width <= 0 || height <= 0)
            return [0, 0, 1, 1];
        const [pw, ph] = this._pixels;
        const scale = Math.max(width / Math.max(1, pw), height / Math.max(1, ph));
        const du = Math.min(1, width / (pw * scale));
        const dv = Math.min(1, height / (ph * scale));
        return [(1 - du) / 2, (1 - dv) / 2, (1 + du) / 2, (1 + dv) / 2];
    }

    vfunc_paint_node(root, paintContext) {
        const texture = this._content?.get_texture();
        if (!texture)
            return;
        const context = paintContext.get_framebuffer().get_context();
        if (this._context !== context) {
            this._pipeline = SHADER.pipeline(context);
            this._context = context;
        }
        const pipeline = this._pipeline;
        pipeline.set_layer_texture(0, texture);
        const box = this.get_allocation_box();
        const [width, height] = box.get_size();
        const crop = this._crop(width, height);
        SHADER.set(pipeline, 'crop', ...crop);
        SHADER.set(pipeline, 'size', width, height);
        SHADER.set(pipeline, 'radius', Math.min(this._radius, width / 2, height / 2));
        // Premultiplied: the whole colour fades with the actor
        const alpha = this.get_paint_opacity() / 255;
        const colour = new Cogl.Color();
        colour.init_from_4f(alpha, alpha, alpha, alpha);
        pipeline.set_color(colour);
        const node = new Clutter.PipelineNode(pipeline);
        root.add_child(node);
        node.add_texture_rectangle(new Clutter.ActorBox({x1: 0, y1: 0, x2: width, y2: height}), ...crop);
    }

    vfunc_destroy() {
        this._pipeline = null;
        this._context = null;
        this._content = null;
        super.vfunc_destroy();
    }
});
