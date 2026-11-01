// The effect each glass surface carries: before the surface paints itself, the glass is drawn under it, cut from the
// frosted picture of the screen at the very spot the surface shows on, so the frost stays still while the surface
// moves over it. Being part of the surface's own paint, the glass follows it through every animation, fade and clip
// with no work of its own, and costs nothing while the surface is not drawn. A surface fading in or out is clearer as
// it is fainter, and one across two screens is cut from each screen's picture where it lies on that screen.
//
// The glass draws the surface's veil and hairline itself (St draws neither while the surface has glass), so that
// both follow the pane's continuous corners, and so that the veil can be made just thick enough for the surface's
// text over the brightest part of the wallpaper (contrast.js). A surface whose stylesheet draws its own background
// keeps it and gets no veil, one that draws its own border (the hairline and neon styles) gets no hairline, and
// either has circular corners to match what St draws.

import Clutter from 'gi://Clutter';
import GObject from 'gi://GObject';
import Graphene from 'gi://Graphene';
import St from 'gi://St';

import {mix, veil} from './contrast.js';
import {CONTINUOUS, VARIANTS} from './materials.js';

// The corners the shell's own surfaces have: continuous, but where effect.js says circular
export const CORNERS = 'continuous';

// The ground and the accent when the theme gives none (glass.css stands in for them too)
const GROUND = {dark: [0x1e / 255, 0x1e / 255, 0x24 / 255], light: [1, 1, 1]};
const ACCENT = [0x91 / 255, 0x48 / 255, 0xf0 / 255];
const WHITE = [1, 1, 1];

const rgb = color => [color.red / 255, color.green / 255, color.blue / 255];
const LN2 = Math.log(2);

// Where the curve of a corner starts (s) and its exponent (n, 2 a circle), in device pixels, for a corner of radius r
// on a pane whose shorter half side is m
export function corner(r, m, circular) {
    if (r < 0.5)
        return [0, 2];
    if (r >= m - 0.5)
        return [m, 2];
    if (circular)
        return [Math.min(r, m), 2];
    const s = Math.min(CONTINUOUS.reach * r, m);
    return [s, Math.max(2, LN2 / -Math.log(1 - CONTINUOUS.diagonal * r / s))];
}

// A look is what a surface asks of its glass:
//   material  one of a variant's materials (materials.js)
//   radius    the corners, in the stylesheet's pixels, or null for the surface's own border-radius, as the desktop
//             style's stylesheet gives it (themed/shell-styles.css.tpl): the pane then always has the corners
//             GNOME draws the surface with
//   shadow    whether a drop shadow falls outside the outline: the variant's, for the scheme ({alpha, blur, offset}
//             in stylesheet pixels stands for a material that has none), or null
//   ring      the strength of a hairline just outside the outline, or 0
//   areas     (actor) => [[x, y, width, height], ...] in the actor's own pixels, where the glass goes when that is
//             not the whole actor
//   content   (actor) => the opacity to draw the surface itself with over its glass, or -1 for its own
//   fade      (y) => a share of the glass's opacity for a surface whose top is at y on the stage
//   text      (actor) => the actor whose text the glass keeps readable, when that is not the surface itself
//   inClones  whether the glass is drawn when the surface is painted as a copy elsewhere (as the magnifier does)
//   stageOnly whether the glass is left out when the surface is painted on its own, as a window is for a screenshot
//             of it or to be shared: the picture is then the surface bare, at full opacity
export const GlassEffect = GObject.registerClass(
class AmethystoraGlassEffect extends Clutter.Effect {
    _init(glass, look) {
        super._init();
        this._glass = glass;
        this.look = look;
        this.params = null;
        this._pipelines = new Map();
        this._textures = new Map();
        this._pictures = new Set();
        this._colors = null;
        this._veils = new Map();
    }

    // A changed look (a window maximized, the colour scheme switched) shows on the next frame
    update(changes) {
        Object.assign(this.look, changes);
        this._veils.clear();
        this.get_actor()?.queue_redraw();
    }

    // The surface's style changed (a theme, the scheme, a class): its colours are read again on its next paint. Not
    // while the glass goes solid: St fades the solid surface in over the glass, which stays as it was until it is in.
    restyle() {
        if (this._glass.solid && this._colors)
            return;
        this._colors = null;
        this._veils.clear();
    }

    // A new picture (another variant, another wallpaper): the veils are worked out again
    repicture() {
        this._veils.clear();
        this.get_actor()?.queue_redraw();
    }

    // Lets go of the pictures, for an effect taken off its surface
    forget() {
        for (const picture of this._pictures)
            picture.consumers.delete(this);
        this._pictures.clear();
        this._pipelines = new Map();
        this._textures = new Map();
        this._veils.clear();
        this._colors = null;
    }

    _scale() {
        return St.ThemeContext.get_for_stage(global.stage).scale_factor;
    }

    // The material with the person's tint (glass.js), or the variant's own
    get _material() {
        return this._glass.materialFor?.(this.look.material) ??
            VARIANTS[this._glass.variant]?.materials[this.look.material] ?? null;
    }

    // The drop shadow, as the variant has it for the scheme
    _shadow(material) {
        if (!this.look.shadow)
            return null;
        return material?.shadow?.[this._glass.light ? 'light' : 'dark'] ?? this.look.shadow;
    }

    // How far outside the surface the glass draws: its shadow and its ring
    _margin() {
        const shadow = this._shadow(this._material);
        const reach = shadow ? shadow.blur + Math.abs(shadow.offset) : 0;
        return Math.ceil(Math.max(reach, this.look.ring ? 2 : 0) * this._scale());
    }

    vfunc_modify_paint_volume(volume) {
        const margin = this._margin();
        if (margin > 0) {
            const origin = volume.get_origin();
            volume.set_origin(new Graphene.Point3D({x: origin.x - margin, y: origin.y - margin, z: origin.z}));
            volume.set_width(volume.get_width() + 2 * margin);
            volume.set_height(volume.get_height() + 2 * margin);
        }
        return true;
    }

    // Whether this is a paint of the actor by itself and not a part of the screen: the screen is painted with the
    // region being redrawn, and a paint of one actor into its own picture has none
    _alone(paintContext) {
        return !!this.look.stageOnly && typeof paintContext.get_redraw_clip === 'function' &&
            !paintContext.get_redraw_clip();
    }

    vfunc_paint_node(node, paintContext, _flags) {
        const actor = this.get_actor();
        let content = -1;
        if (!this._alone(paintContext) && (this.look.inClones || !actor.is_in_clone_paint()) &&
            this._paintGlass(node, paintContext, actor))
            content = this.look.content?.(actor) ?? -1;
        node.add_child(new Clutter.ActorNode(actor, content));
    }

    // The colours the glass takes from the surface's stylesheet, read once per style: the text, the ground the veil
    // is made of, the accent, a hairline the stylesheet hands the glass, and whether St draws a background or a
    // border of its own (then the glass leaves that out, and its corners are St's circular ones)
    _readColors(actor) {
        if (this._colors)
            return this._colors;
        const light = this._glass.light;
        const colors = {
            own: false, ownBackground: false, ownBorder: false, fill: null,
            text: null, ground: GROUND[light ? 'light' : 'dark'], glow: null, edge: null,
        };
        const node = actor instanceof St.Widget ? actor.get_theme_node() : null;
        if (node) {
            const background = node.get_background_color();
            colors.ownBackground = background.alpha > 0;
            if (colors.ownBackground)
                colors.fill = [...rgb(background), background.alpha / 255];
            for (const side of [St.Side.TOP, St.Side.RIGHT, St.Side.BOTTOM, St.Side.LEFT]) {
                if (node.get_border_width(side) > 0 && node.get_border_color(side).alpha > 0)
                    colors.ownBorder = true;
            }
            colors.own = colors.ownBackground || colors.ownBorder;
            const textActor = this.look.text?.(actor) ?? actor;
            const textNode = textActor instanceof St.Widget ? textActor.get_theme_node() : node;
            colors.text = rgb(textNode.get_foreground_color());
            const [hasGround, ground] = node.lookup_color('-amethystora-glass-ground', true);
            if (hasGround)
                colors.ground = rgb(ground);
            const [hasAccent, accent] = node.lookup_color('-amethystora-glass-accent', true);
            colors.glow = mix(hasAccent ? rgb(accent) : ACCENT, WHITE, 0.65);
            const [hasEdge, edge] = node.lookup_color('-amethystora-glass-edge', false);
            if (hasEdge)
                colors.edge = [...rgb(edge), edge.alpha / 255];
        }
        colors.glow ??= mix(ACCENT, WHITE, 0.65);
        this._colors = colors;
        return colors;
    }

    // The veil over one screen's picture: the variant's own, thickened where the surface's text would not read. Under
    // a background St draws itself (a bar piece of a style) the glass has no veil of its own, but the guard still
    // adds what that background leaves wanting; under an opaque one it has nothing to do. box is where the surface
    // is on the screen, which a bare bar is judged by.
    _veilFor(picture, material, colors, box) {
        const fill = colors.ownBackground ? colors.fill : null;
        const base = fill ? 0 : material.veil[this._glass.light ? 1 : 0];
        if (material.guard === null || !colors.text || !picture.stats || (fill && fill[3] >= 0.99))
            return base;
        const where = material.bare ? box.map(Math.round).join(',') : '';
        const key = `${picture.stats.version}:${this._glass.variant}:${this.look.material}:${this._glass.light}:${where}`;
        let value = this._veils.get(key);
        if (value === undefined) {
            const {hi, lo} = material.bare
                ? picture.extremesIn(this._glass.variant, this.look.material, material, box)
                : picture.extremes(this._glass.variant, this.look.material, material);
            value = hi ? veil({text: colors.text, ground: colors.ground, hi, lo, base, allowance: material.guard, fill})
                : base;
            this._veils.set(key, value);
        }
        return value;
    }

    _paintGlass(node, paintContext, actor) {
        const look = this.look;
        const material = this._material;
        const width = actor.width;
        const height = actor.height;
        if (!material || width < 1 || height < 1)
            return false;
        // Where the actor is on the stage, as drawn: through every transformation of it and its parents
        const extents = actor.get_transformed_extents();
        const sx = extents.size.width / width;
        const sy = extents.size.height / height;
        const opacity = actor.get_paint_opacity() / 255;
        if (opacity <= 0 || sx <= 0 || sy <= 0)
            return false;

        const scale = this._scale();
        // A surface stretched along one axis (the dock's pill while it swells) keeps round corners: lengths follow
        // the smaller of its two scales
        const even = Math.min(sx, sy);
        const unit = scale * even;
        const margin = this._margin();
        const areas = look.areas?.(actor) ?? [[0, 0, width, height]];
        const widget = actor instanceof St.Widget;
        // The theme node's lengths are already in device pixels
        const radius = look.radius ??
            (widget ? actor.get_theme_node().get_border_radius(St.Corner.TOPLEFT) : 0) / scale;
        const colors = this._readColors(actor);
        const circular = look.material === 'pane' || colors.own;
        const shadow = this._shadow(material);
        const frost = this._glass.frost;
        if (!frost)
            return false;
        let drawn = false;
        this.params = null;
        areas.forEach(([x, y, w, h], i) => {
            if (w < 1 || h < 1)
                return;
            const left = extents.origin.x + x * sx;
            const top = extents.origin.y + y * sy;
            const fade = look.fade?.(top) ?? 1;
            if (fade <= 0)
                return;
            const size = [w * sx, h * sy];
            const [s, n] = corner(radius * unit, Math.min(size[0], size[1]) / 2, circular);
            // The quad covers the surface and its shadow, here on the stage. Each screen it reaches draws its own part
            // from its own picture, so that a pane across two screens shows on each the frost of that screen's
            // wallpaper; the lens is worked out for the whole pane, so the parts meet without a seam.
            const quad = [left - margin * sx, top - margin * sy, left + (w + margin) * sx, top + (h + margin) * sy];
            for (const picture of frost.pictures(material.picture)) {
                const monitor = picture.monitor;
                const x1 = Math.max(quad[0], monitor.x);
                const y1 = Math.max(quad[1], monitor.y);
                const x2 = Math.min(quad[2], monitor.x + monitor.width);
                const y2 = Math.min(quad[3], monitor.y + monitor.height);
                if (x2 <= x1 || y2 <= y1)
                    continue;
                // Drawn again whenever the picture changes, ready or not
                picture.consumers.add(this);
                this._pictures.add(picture);
                const ready = picture.ready && picture.texture;
                // A window has nothing to stand in for its glass: it is drawn opaque until the picture is there
                if (!ready && !widget)
                    continue;
                // As far as the surface is see-through while it fades, its frost is clear too
                const clear = frost.picture(monitor.index, 'clear');
                const sharp = ready && clear?.ready && clear.texture ? clear.texture : null;
                const pipeline = this._pipeline(paintContext, `${i}:${monitor.index}`,
                    ready ? picture.texture : null, sharp);
                const veilAlpha = ready ? this._veilFor(picture, material, colors, quad) : 0;
                // A bare bar draws nothing at all while its text reads without help
                if (material.bare && (!ready || veilAlpha < 0.005))
                    continue;
                this._set(pipeline, {
                    monitor, left, top, size, s, n, unit, opacity: opacity * fade, sharp, material, colors, shadow,
                    veilAlpha, solid: !ready,
                });
                // What the first pane drawn was made of, for the tests to read
                if (!this.params) {
                    this.params = {
                        variant: this._glass.variant, material: look.material, corner: [s, n],
                        veil: veilAlpha, ground: colors.ground, solid: !ready, circular,
                    };
                }

                // The part in the actor's own pixels, and where it falls on its screen
                const [ux, uy] = [1 / monitor.width, 1 / monitor.height];
                const box = new Clutter.ActorBox({
                    x1: x - margin + (x1 - quad[0]) / sx,
                    y1: y - margin + (y1 - quad[1]) / sy,
                    x2: x - margin + (x2 - quad[0]) / sx,
                    y2: y - margin + (y2 - quad[1]) / sy,
                });
                const pipelineNode = new Clutter.PipelineNode(pipeline);
                node.add_child(pipelineNode);
                pipelineNode.add_texture_rectangle(box,
                    (x1 - monitor.x) * ux, (y1 - monitor.y) * uy, (x2 - monitor.x) * ux, (y2 - monitor.y) * uy);
                drawn = true;
            }
        });
        return drawn;
    }

    // Every uniform of one pane on one screen. Lengths are device pixels: stylesheet pixels times the scale times
    // the surface's own scale on screen.
    _set(pipeline, {monitor, left, top, size, s, n, unit, opacity, sharp, material: m, colors, shadow, veilAlpha,
        solid}) {
        const shader = this._glass.shader;
        const [ux, uy] = [1 / monitor.width, 1 / monitor.height];
        shader.set(pipeline, 'u_origin', (left - monitor.x) * ux, (top - monitor.y) * uy);
        shader.set(pipeline, 'u_pixel', ux, uy);
        shader.set(pipeline, 'u_size', size[0], size[1]);
        shader.set(pipeline, 'u_corner', s, n);
        shader.set(pipeline, 'u_opacity', opacity);
        shader.set(pipeline, 'u_frost', sharp ? opacity : 1);
        shader.set(pipeline, 'u_solid', solid ? 0.92 : m.bare ? veilAlpha : 0);
        shader.set(pipeline, 'u_tone', m.sat, m.dim, m.tint, m.glint);
        shader.set(pipeline, 'u_rim', m.rim[0] * unit, Math.max(m.rim[1] * unit, 1e-3), m.rim[2], m.rim[3]);
        shader.set(pipeline, 'u_shade', m.shade);
        shader.set(pipeline, 'u_veil', ...colors.ground, veilAlpha);
        if (colors.ownBorder || !colors.text)
            shader.set(pipeline, 'u_edge', 0, 0, 0, 0);
        else if (colors.edge)
            shader.set(pipeline, 'u_edge', ...colors.edge);
        else
            shader.set(pipeline, 'u_edge', ...colors.text, m.edge);
        shader.set(pipeline, 'u_lift', ...m.lift ?? [0, 0]);
        shader.set(pipeline, 'u_light', m.top?.[0] ?? 0, (m.top?.[1] ?? 1) * unit, m.sides?.[0] ?? 0,
            m.sides?.[1] ?? 0);
        shader.set(pipeline, 'u_ring', m.ring);
        const glow = m.glow;
        shader.set(pipeline, 'u_glow', ...colors.glow, glow?.alpha ?? 0);
        const side = m.sideGlow;
        shader.set(pipeline, 'u_glowshape', (glow?.y ?? 0) * unit, Math.max((glow?.blur ?? 2) / 2 * unit, 1e-3),
            (glow?.grow ?? 0) * unit, side?.alpha ?? 0);
        shader.set(pipeline, 'u_sideglow', (side?.x ?? 0) * unit, Math.max((side?.blur ?? 2) / 2 * unit, 1e-3),
            (side?.grow ?? 0) * unit, unit);
        shader.set(pipeline, 'u_sheen', ...m.sheen ?? [0, 0.5, 0.5, 0]);
        shader.set(pipeline, 'u_shadow',
            shadow ? shadow.alpha : 0,
            shadow ? shadow.blur * unit : 0,
            shadow ? shadow.offset * unit : 0,
            this.look.ring ?? 0);
    }

    // One pipeline for each area and screen, with that screen's pictures: the frosted one, and the sharp one when
    // there is one (else the frosted one again, which the shader then never mixes in). With no picture yet, the
    // layers keep what they had, which the shader does not sample.
    _pipeline(paintContext, key, texture, sharp) {
        const shader = this._glass.shader;
        let pipeline = this._pipelines.get(key);
        if (!pipeline) {
            pipeline = shader.pipeline(paintContext.get_framebuffer().get_context());
            shader.setInt(pipeline, 'u_picture', 0);
            shader.setInt(pipeline, 'u_clear', 1);
            this._pipelines.set(key, pipeline);
        }
        if (!texture)
            return pipeline;
        // Only a changed picture makes a new pair: this runs on every paint
        const clear = sharp ?? texture;
        const held = this._textures.get(key);
        if (held?.[0] !== texture || held?.[1] !== clear) {
            if (held?.[0] !== texture)
                pipeline.set_layer_texture(0, texture);
            if (held?.[1] !== clear)
                pipeline.set_layer_texture(1, clear);
            this._textures.set(key, [texture, clear]);
        }
        return pipeline;
    }
});
