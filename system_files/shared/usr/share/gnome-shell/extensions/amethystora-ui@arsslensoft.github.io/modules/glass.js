// The glass: the shell drawn on panes of the wallpaper. The top bar is a clear strip of it until a window touches the
// bar; the dock, every menu and popup, the dialogs and the application windows that are not being used stand on
// panes of glass with a rim along their edges; the overview pushes the wallpaper far back behind a darker frost, and
// app folders open over a live blur of the app grid. The glass is a still picture of the desktop cut to each surface,
// never a view of the windows behind it: blurred once per screen, it costs nothing while nothing changes. Not on the
// lock screen, where GNOME's own blur stays.
//
// Three kinds of glass can be picked (ame desktop glass, Control): frost, prism (the default) and clear
// (glass/materials.js). The shell's own surfaces have continuous corners, and their text keeps 4.5:1 against the
// glass on any wallpaper. The glass is solid, the theme's own surfaces with nothing blurred and every picture freed,
// while Reduce transparency is on (ame desktop comfort reduce-transparency), while GNOME's High Contrast is on (with
// stronger outlines, themed/shell-glass.css.tpl) and while the power saver is on (the comfort module watches both).
//
// The frosted pictures are in glass/frost.js, the effect that draws a pane under a surface in glass/effect.js with
// its shader in glass/lens.glsl; the parts that find GNOME's surfaces are glass/popups.js, glass/topbar.js,
// glass/windows.js and glass/backdrops.js.

import GLib from 'gi://GLib';
import St from 'gi://St';
import System from 'system';

import * as Main from 'resource:///org/gnome/shell/ui/main.js';

import {Backdrops} from './glass/backdrops.js';
import {GlassEffect} from './glass/effect.js';
import {Frost} from './glass/frost.js';
import {DEFAULT_VARIANT, VARIANTS} from './glass/materials.js';
import {Popups} from './glass/popups.js';
import {Shader} from './glass/shader.js';
import {TopBar} from './glass/topbar.js';
import {Windows} from './glass/windows.js';
import {setClasses} from './theme/classes.js';

const SCHEMA = 'org.gnome.shell.extensions.amethystora-ui.glass';
const EFFECT = 'amethystora-glass';
const LIGHT = 'amethystora-glass-light';
// Every St surface with glass carries it: it holds the theme's ground and accent for the glass to read
const PANE = 'amethystora-glass-pane';
const SOLID = 'amethystora-glass-solid';
const CONTRAST = 'amethystora-glass-contrast';
// How long St takes to bring the solid surfaces in (themed/shell-glass.css.tpl), in milliseconds before the slow-down
// factor, and a frame more: the glass stays under them until then
const SOLID_FADE = 300;
const FRAME = 32;

export class Glass {
    constructor(ext) {
        this._ext = ext;
    }

    enable() {
        this._surfaces = new Map();
        this._moduleSurfaces = new Set();
        this._wrapped = [];
        this._parts = [];

        const file = this._ext.dir.get_child('modules').get_child('glass').get_child('lens.glsl');
        const [, bytes] = file.load_contents(null);
        const lens = new TextDecoder().decode(bytes);
        this.shader = new Shader(lens, 'cogl_color_out = glass_color();', 2);
        // The inside of a large pane, where only the picture, its tone and its wash show (effect.js)
        this.interior = new Shader(lens, 'cogl_color_out = glass_interior();', 2);

        this._settings = this._ext.getSettings(SCHEMA);
        this.variant = this._readVariant();
        this._strengths = new Map();
        this._materials = new Map();
        // Counts every change the glass's panes draw differently after, which effect.js gives the native paint again
        this.version = 0;
        this._settings.connectObject(
            'changed::material', () => this._syncVariant(),
            'changed::solid', () => this._syncSolid(),
            this);
        for (const variant of Object.keys(VARIANTS)) {
            this._settings.connectObject(
                `changed::blur-${variant}`, () => this._syncBlur(variant),
                `changed::tint-${variant}`, () => this._syncTint(variant),
                this);
        }
        this._unlisten = this.comfort?.listen(() => this._syncSolid()) ?? null;
        this._freeId = 0;
        // Comfort learns the power profile a moment after it starts: logging in with the power saver on builds the
        // glass here and lets it go a few milliseconds later, which costs only that once
        this._solid = this._wantsSolid();
        this.frost = this._solid ? null : this._newFrost();
        this._syncClasses();

        this.light = Main.getStyleVariant() === 'light';
        St.Settings.get().connectObject('notify::color-scheme', () => this._syncScheme(), this);
        // Read by every pane on every frame: kept here rather than asked of St each time
        const context = St.ThemeContext.get_for_stage(global.stage);
        this.scale = context.scale_factor;
        context.connectObject('notify::scale-factor', () => {
            this.scale = context.scale_factor;
            this.version++;
        }, this);

        // The windows first: the top bar and the workspace slide ask them
        this.windows = new Windows(this);
        this._topBar = new TopBar(this);
        this._backdrops = new Backdrops(this);
        this._parts = [this.windows, new Popups(this), this._topBar, this._backdrops];
        this._started = new Set();
        for (const part of this._parts) {
            // The windows and the backdrops have nothing to do while the glass is solid
            if (!this._solid || !this._paused(part))
                this._start(part);
        }
        this._running = true;
        // A new desktop style, or new buttons on a bar whose buttons each have glass. The menus, banners, dialogs
        // and the dock take their corners from the stylesheet as they are drawn, and need nothing.
        this.style?.onChange(() => {
            if (!this._running)
                return;
            this.version++;
            this.windows?.restyle();
            this._topBar?.restyle();
        });
        this.surfacesChanged();
    }

    // The desktop style module, when it runs
    get style() {
        return this._ext.modules.get('style') ?? null;
    }

    // The comfort module, which says when High Contrast or the power saver is on
    get comfort() {
        return this._ext.modules.get('comfort') ?? null;
    }

    // The glass picked, by name (frost, prism or clear)
    get material() {
        return this.variant;
    }

    // Whether the glass is drawn solid now, for whichever reason
    get solid() {
        return this._solid ?? false;
    }

    // The corners of a window's pane: the desktop style's, which its GTK stylesheet gives the windows too
    get windowRadius() {
        return this.style?.look.window ?? 16;
    }

    disable() {
        this._running = false;
        this._unlisten?.();
        this._unlisten = null;
        this._settings?.disconnectObject(this);
        // Disposed now rather than by the collector (dock.js says why)
        this._settings?.run_dispose();
        this._settings = null;
        if (this._freeId)
            GLib.source_remove(this._freeId);
        this._freeId = 0;
        St.Settings.get().disconnectObject(this);
        St.ThemeContext.get_for_stage(global.stage).disconnectObject(this);
        for (const part of [...this._parts ?? []].reverse())
            this._stop(part);
        this._parts = [];
        this._started = null;
        this.windows = null;
        this._topBar = null;
        this._backdrops = null;
        // GNOME's methods back as they were, the last wrapped first
        for (const [proto, name, descriptor] of [...this._wrapped ?? []].reverse()) {
            if (descriptor)
                Object.defineProperty(proto, name, descriptor);
            else
                delete proto[name];
        }
        this._wrapped = [];
        for (const actor of [...this._surfaces?.keys() ?? []])
            this.detach(actor);
        this._surfaces = null;
        this._moduleSurfaces = null;
        this.frost?.destroy();
        this.frost = null;
        this._lastStats = null;
        this.shader?.destroy();
        this.shader = null;
        this.interior?.destroy();
        this.interior = null;
        // Kept through a screen lock (style.js says why); the unlock's enable puts them right
        const group = Main.layoutManager.uiGroup;
        if (!Main.sessionMode.isLocked) {
            group.remove_style_class_name(SOLID);
            group.remove_style_class_name(CONTRAST);
        }
        this._solid = false;
    }

    // The other modules' own surfaces (the dock's pill): asked for again whenever one of them rebuilds its actors.
    // The frame stops the glass before the dock, which still calls this as it goes.
    surfacesChanged() {
        if (!this._running)
            return;
        // The other screens' top bars (modules/bars.js) are panes of the top bar's kind
        this._topBar?.restyle();
        const listed = new Set();
        for (const module of this._ext.modules.values()) {
            for (const {actor, material, radius} of module.glassSurfaces?.() ?? []) {
                listed.add(actor);
                this.attach(actor, {material, radius});
            }
        }
        for (const actor of this._moduleSurfaces) {
            if (!listed.has(actor))
                this.detach(actor);
        }
        this._moduleSurfaces = listed;
    }

    // Gives an actor its pane, or a new look for the pane it has, and returns its effect (null while the glass is
    // solid: the surface is then only given its classes, and its pane once the glass is clear again). classes are
    // style classes it carries while it has one; with scheme, it also carries the light scheme's class and its shadow
    // follows the scheme. also: classes of the caller's own put on in the same change (one restyle, theme/classes.js).
    attach(actor, look, {classes = [], scheme = false, also = []} = {}) {
        let surface = this._surfaces.get(actor);
        if (surface) {
            // Drawn again only when the look changed: every focus change attaches every window's pane again
            if (Object.keys(look).some(key => surface.look[key] !== look[key])) {
                Object.assign(surface.look, look);
                surface.effect?.update({});
            }
            return surface.effect;
        }
        const widget = actor instanceof St.Widget;
        surface = {look, effect: null, classes: widget ? [...classes, PANE] : classes, scheme};
        this._surfaces.set(actor, surface);
        if (widget) {
            const light = scheme && this.light;
            setClasses(actor, [...surface.classes, ...light ? [LIGHT] : [], ...also], scheme && !light ? [LIGHT] : []);
        }
        if (!this._solid)
            this._addEffect(actor, surface);
        if (scheme)
            this._schemeOf(actor, surface);
        actor.connectObject('destroy', () => {
            surface.effect?.forget();
            this._surfaces?.delete(actor);
        }, this);
        // The colours the glass reads from the surface's stylesheet are read again when it changes
        if (widget)
            actor.connectObject('style-changed', () => surface.effect?.restyle(), this);
        return surface.effect;
    }

    // also: classes of the caller's own taken off in the same change
    detach(actor, {also = []} = {}) {
        const surface = this._surfaces?.get(actor);
        if (!surface)
            return;
        this._surfaces.delete(actor);
        actor.disconnectObject(this);
        this._removeEffect(actor, surface);
        // Only widgets were given classes: a window's actor is not one
        if (actor instanceof St.Widget)
            setClasses(actor, [], [...surface.classes, ...surface.scheme ? [LIGHT] : [], ...also]);
    }

    _addEffect(actor, surface) {
        surface.effect = new GlassEffect(this, surface.look);
        actor.add_effect_with_name(EFFECT, surface.effect);
    }

    _removeEffect(actor, surface) {
        if (!surface.effect)
            return;
        actor.remove_effect(surface.effect);
        surface.effect.forget();
        surface.effect = null;
    }

    // A GNOME method with the glass's work around it: around(original, instance, args) is called in its place, and
    // disable() puts the method back
    wrap(proto, name, around) {
        const original = proto[name];
        this._wrapped.push([proto, name, Object.getOwnPropertyDescriptor(proto, name)]);
        proto[name] = function (...args) {
            return around(original, this, args);
        };
    }

    has(actor) {
        return this._surfaces?.has(actor) ?? false;
    }

    effectOf(actor) {
        return this._surfaces?.get(actor)?.effect ?? null;
    }

    // The soft shadow under a popup, as the variant has it for the scheme
    get shadow() {
        const material = VARIANTS[this.variant ?? DEFAULT_VARIANT].materials.glass;
        return material.shadow[this.light ? 'light' : 'dark'];
    }

    _readVariant() {
        const name = this._settings.get_string('material');
        return VARIANTS[name] ? name : DEFAULT_VARIANT;
    }

    _newFrost() {
        return new Frost(this.recipeFor(this.variant), () => ({
            variant: this.variant,
            material: this.materialFor('glass'),
        }), this._lastStats);
    }

    // ---- How deep each glass's blur is and how dense its tint (R6-28) ----

    // A share of the glass's own, from the key, held to its range
    _strength(kind, variant) {
        const value = this._settings?.get_double(`${kind}-${variant}`) ?? 1;
        return Number.isFinite(value) ? Math.min(1.5, Math.max(0.5, value)) : 1;
    }

    // The variant's recipe with its blur scaled: the same pass, a longer or shorter radius
    recipeFor(variant) {
        const recipe = VARIANTS[variant].recipe;
        return {...recipe, radius: Math.max(1, Math.round(recipe.radius * this._strength('blur', variant)))};
    }

    // One of the variant's materials with its tint and veil scaled. The guard (contrast.js) still thickens the veil
    // wherever text would not read, so the floor of 4.5:1 holds at every strength.
    materialFor(name) {
        // Asked by every pane on every frame: worked out once for each variant and strength
        let material = this._materials?.get(name);
        if (material === undefined) {
            material = this._scaledMaterial(name);
            (this._materials ??= new Map()).set(name, material);
        }
        return material;
    }

    _scaledMaterial(name) {
        const variant = this.variant ?? DEFAULT_VARIANT;
        const material = VARIANTS[variant]?.materials[name] ?? null;
        const tint = this._strength('tint', variant);
        if (!material || tint === 1)
            return material;
        const key = `${variant}:${tint}:${name}`;
        let scaled = this._strengths.get(key);
        if (!scaled) {
            scaled = {
                ...material,
                tint: Math.min(0.9, material.tint * tint),
                veil: material.veil.map(v => Math.min(0.95, v * tint)),
            };
            this._strengths.set(key, scaled);
        }
        return scaled;
    }

    _syncBlur(variant) {
        this.version++;
        if (variant === this.variant)
            this.frost?.setRecipe('glass', this.recipeFor(variant));
    }

    _syncTint(variant) {
        this.version++;
        this._strengths.clear();
        this._materials?.clear();
        if (variant !== this.variant)
            return;
        for (const picture of this.frost?.pictures('glass') ?? [])
            picture.forgetExtremes?.();
        for (const [actor, surface] of this._surfaces) {
            surface.effect?.repicture();
            actor.queue_redraw();
        }
    }

    // Another kind of glass: its pictures are blurred again (the old ones shown until the new ones are ready), and
    // every pane drawn again with its materials
    _syncVariant() {
        const variant = this._readVariant();
        if (variant === this.variant)
            return;
        this.version++;
        this.variant = variant;
        this._materials?.clear();
        this.frost?.setRecipe('glass', this.recipeFor(variant));
        for (const [actor, surface] of this._surfaces) {
            if (surface.scheme && surface.look.shadow)
                surface.look.shadow = this.shadow;
            surface.effect?.update({});
            surface.effect?.queue_repaint();
            // Another variant may cast a longer shadow: the volume Clutter keeps for the actor is measured again
            actor.invalidate_paint_volume?.();
            actor.queue_redraw();
        }
    }

    _wantsSolid() {
        const comfort = this.comfort;
        // The power saver, or a focus profile asking for what it does (comfort's lighter)
        return !!this._settings?.get_boolean('solid') || !!comfort?.highContrast ||
            !!(comfort?.lighter ?? comfort?.powerSaver);
    }

    _paused(part) {
        return part === this.windows || part === this._backdrops;
    }

    _start(part) {
        if (this._started.has(part))
            return;
        this._started.add(part);
        part.enable();
    }

    _stop(part) {
        if (!this._started?.has(part))
            return;
        this._started.delete(part);
        part.disable();
    }

    _syncClasses() {
        const group = Main.layoutManager.uiGroup;
        if (this._solid)
            group.add_style_class_name(SOLID);
        else
            group.remove_style_class_name(SOLID);
        if (this.comfort?.highContrast)
            group.add_style_class_name(CONTRAST);
        else
            group.remove_style_class_name(CONTRAST);
    }

    // Solid while Reduce transparency, High Contrast or the power saver is on. Going solid, St fades the theme's
    // surfaces in over the glass, which stays under them for as long as that takes, so that nothing behind an open
    // menu shows for a moment; then every picture and every pane is freed, and the GPU memory goes at once. Coming
    // back needs no fade: the panes are made again at once, each drawing the theme's ground, close to the solid colour
    // it had, for the frame or two until its picture is ready.
    _syncSolid() {
        if (!this._running)
            return;
        const solid = this._wantsSolid();
        if (solid === this._solid) {
            this._syncClasses();
            return;
        }
        this._solid = solid;
        if (this._freeId)
            GLib.source_remove(this._freeId);
        this._freeId = 0;
        if (solid) {
            this._syncClasses();
            this._topBar?.restyle();
            const settings = St.Settings.get();
            const fade = settings.enable_animations ? SOLID_FADE * settings.slow_down_factor + FRAME : 0;
            this._freeId = GLib.timeout_add(GLib.PRIORITY_DEFAULT, Math.ceil(fade), () => {
                this._freeId = 0;
                this._free();
                return GLib.SOURCE_REMOVE;
            });
        } else {
            this.frost ??= this._newFrost();
            this._syncClasses();
            for (const [actor, surface] of this._surfaces) {
                if (!surface.effect)
                    this._addEffect(actor, surface);
            }
            for (const part of this._parts) {
                if (this._paused(part))
                    this._start(part);
            }
            this._topBar?.restyle();
        }
    }

    // What the glass holds, let go of once the solid surfaces are in
    _free() {
        if (!this._running || !this._solid)
            return;
        for (const part of this._parts) {
            if (this._paused(part))
                this._stop(part);
        }
        for (const [actor, surface] of this._surfaces)
            this._removeEffect(actor, surface);
        // A few numbers per screen, kept so that the glass comes back with the veil its text needs
        this._lastStats = this.frost?.stats() ?? null;
        this.frost?.destroy();
        this.frost = null;
        // The pictures' wrappers let go of the GPU's memory now rather than at the next collection
        System.gc();
    }

    _schemeOf(actor, surface) {
        if (this.light)
            actor.add_style_class_name(LIGHT);
        else
            actor.remove_style_class_name(LIGHT);
        if (surface.look.shadow) {
            surface.look.shadow = this.shadow;
            surface.effect?.update({});
            actor.invalidate_paint_volume?.();
        }
    }

    _syncScheme() {
        const light = Main.getStyleVariant() === 'light';
        if (light === this.light)
            return;
        this.light = light;
        this.version++;
        for (const [actor, surface] of this._surfaces) {
            if (surface.scheme)
                this._schemeOf(actor, surface);
            surface.effect?.restyle();
        }
    }
}
