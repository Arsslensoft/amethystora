// The frosted pictures the glass is cut from: for each screen and each strength of frost, the desktop's wallpaper as
// that screen shows it, blurred once and kept in a small texture that every surface on the screen shares. Nothing is
// blurred again until the wallpaper or the screens change.
//
// A picture is made by an actor that is never shown: the screen's wallpaper, with GNOME's blur on it and, around
// that, an effect that renders the result into the texture. A one-pixel clone of that actor in the corner of its
// screen is what gets it painted: Clutter redraws a clone whenever the actor it copies changes, so the wallpaper
// changing (or fading into another) is all it takes, and the clone itself draws nothing.
//
// Half a second after the glass's picture of a screen has stopped changing, it is read back once, small, and kept
// as samples from which each surface works out how thick its veil must be for its text (effect.js, contrast.js).

import Clutter from 'gi://Clutter';
import Cogl from 'gi://Cogl';
import GdkPixbuf from 'gi://GdkPixbuf';
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import GObject from 'gi://GObject';
import Graphene from 'gi://Graphene';
import Meta from 'gi://Meta';
import Shell from 'gi://Shell';
import St from 'gi://St';

import * as Background from 'resource:///org/gnome/shell/ui/background.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';

import {extremes} from './contrast.js';

// The strengths of frost: the blur's radius in pixels (times the screen's scale), the brightness after it, and how
// many times smaller than the screen the texture is across. The glass is drawn from the first, whose recipe is the
// chosen variant's (materials.js; frost's is the one here), the overview's backdrop from the second; the third is the
// wallpaper itself, which a pane fading in or out is mixed towards. Blurred this much, nothing is lost by a quarter of
// the screen; the sharp one is kept at half, and seen only faintly.
export const RECIPES = {
    glass: {radius: 48, brightness: 1.0, shrink: 4},
    frost: {radius: 64, brightness: 0.55, shrink: 4},
    clear: {radius: 0, brightness: 1.0, shrink: 2},
};

// How long a picture has to stay unchanged before it is read back, in milliseconds, and the size it is read at
const SETTLE = 500;
const CONFIRM = 3000;
const SAMPLES = [64, 36];
let statsVersion = 0;

// Renders the actor it is on, blurred by the effect after it, into the picture's texture whenever the actor has
// changed, and draws nothing on the screen
const Capture = GObject.registerClass(
class AmethystoraGlassCapture extends Clutter.Effect {
    _init(picture) {
        super._init();
        this._picture = picture;
    }

    vfunc_paint_node(node, paintContext, flags) {
        const picture = this._picture;
        if (picture.ready && !picture.dirty && !(flags & Clutter.EffectPaintFlags.ACTOR_DIRTY))
            return;
        const framebuffer = picture.framebuffer(paintContext.get_framebuffer().get_context());
        if (!framebuffer)
            return;

        const actor = this.get_actor();
        const layer = Clutter.LayerNode.new_to_framebuffer(framebuffer, picture.blank);
        node.add_child(layer);
        const scale = new Graphene.Matrix();
        scale.init_scale(picture.width / actor.width, picture.height / actor.height, 1);
        const transform = new Clutter.TransformNode(scale);
        layer.add_child(transform);
        transform.add_child(new Clutter.ActorNode(actor, 255));
        picture.rendered();
    }
});

class Picture {
    constructor(frost, holder, monitor, name, recipe) {
        this.recipe = name;
        this._frost = frost;
        this.monitor = monitor;
        this.consumers = new Set();
        this.texture = null;
        this.ready = false;
        this.dirty = false;
        this.stats = null;
        this._extremes = new Map();
        this._settleId = 0;
        this._confirmId = 0;
        this._confirmed = false;
        this._sizeFor(recipe);

        const scale = St.ThemeContext.get_for_stage(global.stage).scale_factor;
        this._source = new Clutter.Actor({visible: false, width: monitor.width, height: monitor.height});
        // Backgrounds of its own rather than a copy of the desktop's: the window manager leaves out the parts of
        // the desktop's that windows cover, which would be holes in the frost. The wallpaper itself is GNOME's
        // shared copy, and a new one is put in as GNOME puts it on the desktop.
        this._backgrounds = new Background.BackgroundManager({
            container: this._source,
            monitorIndex: monitor.index,
            controlPosition: false,
        });
        this._source.add_effect(new Capture(this));
        this._blur = null;
        if (recipe.radius > 0) {
            this._blur = new Shell.BlurEffect({
                mode: Shell.BlurMode.ACTOR,
                radius: Math.round(recipe.radius * scale),
                brightness: recipe.brightness,
            });
            this._source.add_effect(this._blur);
        }
        holder.add_child(this._source);

        this._driver = new Clutter.Clone({
            source: this._source,
            x: monitor.x + monitor.width - 1,
            y: monitor.y + monitor.height - 1,
            width: 1,
            height: 1,
        });
        holder.add_child(this._driver);
    }

    _sizeFor(recipe) {
        this.radius = recipe.radius;
        this.brightness = recipe.brightness;
        this.shrink = recipe.shrink;
        this.width = Math.max(1, Math.ceil(this.monitor.width / recipe.shrink));
        this.height = Math.max(1, Math.ceil(this.monitor.height / recipe.shrink));
    }

    // Another recipe (another variant of glass): blurred again on the next frame, into a texture of the new size,
    // with the old picture shown until then, so that nothing flashes
    setRecipe(recipe) {
        if (!this._blur || recipe.radius <= 0)
            return;
        const scale = St.ThemeContext.get_for_stage(global.stage).scale_factor;
        this._sizeFor(recipe);
        this._blur.radius = Math.round(recipe.radius * scale);
        this._blur.brightness = recipe.brightness;
        this.dirty = true;
        this._source.queue_redraw();
    }

    // The texture and the render target over it, made on the first paint, when the GPU is at hand, and again at a
    // new size. Should the GPU refuse them, that screen goes without glass rather than asking again on every frame.
    framebuffer(context) {
        if (this._refused)
            return null;
        if (this._framebuffer && this.texture.get_width() === this.width && this.texture.get_height() === this.height)
            return this._framebuffer;
        try {
            const texture = Cogl.Texture2D.new_with_size(context, this.width, this.height);
            texture.set_premultiplied(true);
            const framebuffer = Cogl.Offscreen.new_with_texture(texture);
            framebuffer.allocate();
            framebuffer.orthographic(0, 0, this.width, this.height, -1, 1);
            this.blank = Cogl.Pipeline.new(context);
            this.texture = texture;
            this._framebuffer = framebuffer;
        } catch (e) {
            console.error(`Amethystora UI: the glass has no picture for a screen: ${e.message}`);
            this._refused = true;
            return null;
        }
        return this._framebuffer;
    }

    flush() {
        this._framebuffer?.flush();
    }

    // Called from the paint that drew a new picture: the surfaces showing it are drawn again before the next frame,
    // and the glass's own picture is read back once it has settled
    rendered() {
        this.ready = true;
        this.dirty = false;
        this._frost.redrawSoon(this);
        if (this.recipe !== 'glass')
            return;
        if (this._settleId)
            GLib.source_remove(this._settleId);
        this._settleId = GLib.timeout_add(GLib.PRIORITY_DEFAULT, SETTLE, () => {
            this._settleId = 0;
            this._readBack();
            return GLib.SOURCE_REMOVE;
        });
    }

    // GNOME's screenshot helper is the one way to read a texture from here: it hands the picture back as a pixbuf
    // (and writes a PNG of it in a thread, which is thrown away)
    _readBack() {
        const texture = this.texture;
        if (!texture || this._destroyed)
            return;
        try {
            Shell.Screenshot.composite_to_stream(texture, 0, 0, -1, -1, 1, null, 0, 0, 1,
                Gio.MemoryOutputStream.new_resizable(), (source, result) => {
                    let pixbuf = null;
                    try {
                        pixbuf = Shell.Screenshot.composite_to_stream_finish(result);
                    } catch (e) {
                        console.error(`Amethystora UI: the glass could not read its picture: ${e.message}`);
                    }
                    if (pixbuf && !this._destroyed && texture === this.texture)
                        this._measure(pixbuf);
                });
        } catch (e) {
            console.error(`Amethystora UI: the glass could not read its picture: ${e.message}`);
        }
    }

    _measure(pixbuf) {
        const [w, h] = SAMPLES;
        const small = pixbuf.scale_simple(w, h, GdkPixbuf.InterpType.TILES);
        const pixels = small.read_pixel_bytes().get_data();
        const stride = small.get_rowstride();
        const channels = small.get_n_channels();
        const samples = new Float32Array(w * h * 3);
        for (let y = 0; y < h; y++) {
            for (let x = 0; x < w; x++) {
                const at = y * stride + x * channels;
                const to = 3 * (y * w + x);
                samples[to] = pixels[at] / 255;
                samples[to + 1] = pixels[at + 1] / 255;
                samples[to + 2] = pixels[at + 2] / 255;
            }
        }
        this._extremes.clear();
        this.stats = {samples, version: ++statsVersion, hi: null, lo: null};
        // The brightest and darkest as the variant's glass shows them, for whoever reads the stats
        const glass = this._frost.glassMaterial?.();
        if (glass)
            Object.assign(this.stats, this.extremes(glass.variant, 'glass', glass.material));
        for (const consumer of this.consumers)
            consumer.repicture?.();
        // The first picture of a session can be read while the wallpaper is still being loaded (seen once in the
        // harness, on a cold start): it is read once more, a little later, the only time this picture is
        if (!this._confirmed) {
            this._confirmed = true;
            this._confirmId = GLib.timeout_add(GLib.PRIORITY_DEFAULT, CONFIRM, () => {
                this._confirmId = 0;
                this._readBack();
                return GLib.SOURCE_REMOVE;
            });
        }
    }

    // The same, of the part of the picture behind a box of the screen ([x1, y1, x2, y2] in its pixels), for a bare
    // bar, which is judged by what lies right behind it
    extremesIn(variant, name, material, box) {
        if (!this.stats)
            return {hi: null, lo: null};
        const [w, h] = SAMPLES;
        const m = this.monitor;
        const cell = (v, size, count) => Math.min(count - 1, Math.max(0, Math.floor(v / size * count)));
        const [c1, r1] = [cell(box[0] - m.x, m.width, w), cell(box[1] - m.y, m.height, h)];
        const [c2, r2] = [cell(box[2] - m.x - 1, m.width, w), cell(box[3] - m.y - 1, m.height, h)];
        const key = `${variant}:${name}:${c1},${r1},${c2},${r2}`;
        let found = this._extremes.get(key);
        if (!found) {
            const part = [];
            for (let r = r1; r <= r2; r++) {
                for (let c = c1; c <= c2; c++)
                    part.push(...this.stats.samples.subarray(3 * (r * w + c), 3 * (r * w + c) + 3));
            }
            found = extremes(part, material) ?? {hi: null, lo: null};
            this._extremes.set(key, found);
        }
        return found;
    }

    // The brightest and darkest parts of the picture as a material of a variant shows them, worked out once for each
    // picture read back
    extremes(variant, name, material) {
        if (!this.stats)
            return {hi: null, lo: null};
        const key = `${variant}:${name}`;
        let found = this._extremes.get(key);
        if (!found) {
            found = extremes(this.stats.samples, material) ?? {hi: null, lo: null};
            this._extremes.set(key, found);
        }
        return found;
    }

    // A material's tint changed (glass.js, R6-28): the extremes are worked out again as they are next asked for
    forgetExtremes() {
        this._extremes.clear();
    }

    // Drawn again from scratch, as after the GPU lost its memory
    refresh() {
        this.ready = false;
        this._source.queue_redraw();
    }

    destroy() {
        this._destroyed = true;
        if (this._settleId)
            GLib.source_remove(this._settleId);
        this._settleId = 0;
        if (this._confirmId)
            GLib.source_remove(this._confirmId);
        this._confirmId = 0;
        this._extremes.clear();
        this.stats = null;
        this._blur = null;
        this._driver.destroy();
        this._backgrounds.destroy();
        this._source.destroy();
        this.consumers.clear();
        this.texture = null;
        this._framebuffer = null;
        this.blank = null;
    }
}

export class Frost {
    // glass: the recipe of the glass's own pictures, the chosen variant's; glassMaterial: () => {variant, material},
    // what the variant's glass is made of, for the brightest and darkest parts of a picture read back; seed: what
    // the last pictures of each screen measured (stats()), which stand until these are measured, so that the glass
    // coming back from solid does not show its thinnest veil for that half second
    constructor(glass = RECIPES.glass, glassMaterial = null, seed = null) {
        this._glassRecipe = glass;
        this.glassMaterial = glassMaterial;
        this._seed = seed;
        this._pictures = new Map();
        this._queued = new Set();
        this._laterId = 0;
        // Under every other actor of the stage, so that a new picture is ready before anything is drawn from it
        this._holder = new Clutter.Actor({name: 'amethystoraGlassPictures'});
        global.stage.insert_child_below(this._holder, Main.layoutManager.uiGroup);
        Main.layoutManager.connectObject('monitors-changed', () => this._rebuild(), this);
        St.ThemeContext.get_for_stage(global.stage).connectObject('notify::scale-factor', () => this._rebuild(), this);
        // GNOME 50 says when the GPU lost what it held, and the pictures are drawn again; GNOME 51 has no such signal
        if (GObject.signal_lookup('gl-video-memory-purged', global.stage.constructor.$gtype)) {
            global.stage.connectObject('gl-video-memory-purged', () => {
                for (const picture of this._pictures.values())
                    picture.refresh();
            }, this);
        }
        this._build();
    }

    // Every picture is made up front, so that none is made during a paint and the overview never opens on a
    // backdrop still to come
    _build() {
        this._byRecipe = null;
        for (const monitor of Main.layoutManager.monitors) {
            for (const name of Object.keys(RECIPES)) {
                const recipe = name === 'glass' ? this._glassRecipe : RECIPES[name];
                const picture = new Picture(this, this._holder, monitor, name, recipe);
                if (name === 'glass')
                    picture.stats = this._seed?.get(monitor.index) ?? null;
                this._pictures.set(`${monitor.index}:${name}`, picture);
            }
        }
    }

    destroy() {
        Main.layoutManager.disconnectObject(this);
        St.ThemeContext.get_for_stage(global.stage).disconnectObject(this);
        global.stage.disconnectObject(this);
        if (this._laterId)
            global.compositor.get_laters().remove(this._laterId);
        this._laterId = 0;
        this._queued.clear();
        for (const picture of this._pictures.values())
            picture.destroy();
        this._pictures.clear();
        this._byRecipe = null;
        this._holder.destroy();
        this._holder = null;
    }

    // What each screen's glass picture measured, by the screen's index
    stats() {
        return new Map(this.pictures('glass').filter(p => p.stats).map(p => [p.monitor.index, p.stats]));
    }

    // A new recipe for the glass's pictures of every screen (the frost and the sharp pictures never change)
    setRecipe(name, recipe) {
        if (name !== 'glass')
            return;
        this._glassRecipe = recipe;
        for (const picture of this.pictures('glass'))
            picture.setRecipe(recipe);
    }

    // The picture of a screen (an index into Main.layoutManager.monitors) for a recipe
    picture(index, recipe) {
        return this._pictures.get(`${index}:${recipe}`) ?? null;
    }

    // The pictures of every screen for a recipe. Asked for on every paint of every pane, so the lists are made once
    // with the pictures rather than each time.
    pictures(recipe) {
        if (!this._byRecipe) {
            this._byRecipe = new Map();
            for (const picture of this._pictures.values()) {
                if (!this._byRecipe.has(picture.recipe))
                    this._byRecipe.set(picture.recipe, []);
                this._byRecipe.get(picture.recipe).push(picture);
            }
        }
        return this._byRecipe.get(recipe) ?? [];
    }

    // Surfaces that drew from a picture are drawn again once it changes. Queued from inside a paint, so it is done
    // just before the next frame. A picture nothing is drawing from (the overview's, while it is closed) is flushed too:
    // Cogl keeps a target's drawing, and every texture that drawing used, until something samples it, which cost
    // megabytes on every new picture until the overview next opened.
    redrawSoon(picture) {
        this._queued.add(picture);
        if (this._laterId)
            return;
        this._laterId = global.compositor.get_laters().add(Meta.LaterType.BEFORE_REDRAW, () => {
            this._laterId = 0;
            for (const queued of this._queued) {
                queued.flush();
                for (const consumer of queued.consumers)
                    consumer.get_actor()?.queue_redraw();
            }
            this._queued.clear();
            return GLib.SOURCE_REMOVE;
        });
    }

    // New screens, or a new scale: every picture is made again, and every surface finds its own on its next paint
    _rebuild() {
        const consumers = new Set();
        for (const picture of this._pictures.values()) {
            for (const consumer of picture.consumers)
                consumers.add(consumer);
            picture.destroy();
        }
        this._pictures.clear();
        this._build();
        for (const consumer of consumers)
            consumer.get_actor()?.queue_redraw();
    }
}
