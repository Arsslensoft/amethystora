// Colour filters: the whole screen drawn through one colour matrix (matrices.js), for an eye that mixes up reds and
// greens or blues and yellows, or in grey. The effect sits on the stage, so that everything a screen shows goes through
// it, Zoom's magnified picture included. Each screen is drawn as always, then what it holds is copied into a texture of
// that screen's own pixels and drawn back over itself through a small fragment shader (a snippet on the texture's Cogl
// pipeline): one copy and one pass of each screen per frame while a filter is on; nothing at all while none is. The
// texture is the screen's own size and scale, so nothing is resampled. A pointer the GPU draws on a plane of its own is
// not filtered; one the shell draws (some virtual machines) is.
//
// Only what is drawn for a screen is filtered. A paint of the stage that is no screen's (a screenshot, the screenshot
// UI's picture, a colour picker's reading, a recording of an area or a window, and some recordings and shares of a whole
// screen, as the program asks for them) has the apps' own colours. While on, fullscreen windows are composited like the
// rest, else a game would skip the filter. A screen in HDR, or one the GPU cannot copy or hold, is left unfiltered, and
// the person is told once.
//
// Other modules lay a matrix of their own over the filter (setExtra: a focus profile's grey), multiplied into the same
// one pass: the effect is there while either is.
//
// A tile in quick settings while a filter is chosen: its switch draws the filter or not without forgetting it, its
// menu picks the filter and how strong it is.

import Clutter from 'gi://Clutter';
import Cogl from 'gi://Cogl';
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import GObject from 'gi://GObject';
import Mtk from 'gi://Mtk';

import {gettext as _} from 'resource:///org/gnome/shell/extensions/extension.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import * as PopupMenu from 'resource:///org/gnome/shell/ui/popupMenu.js';
import {QuickMenuToggle, SystemIndicator} from 'resource:///org/gnome/shell/ui/quickSettings.js';

import {FILTERS, filterMatrix, multiply} from './matrices.js';

export const EFFECT_NAME = 'amethystora-access-filter';
// The strengths the tile offers, as the board's chips
const STRENGTHS = [50, 75, 100];
// What could not be filtered is said once a session, for each reason
const told = new Set();

// sRGB to linear light and back, the matrix in between, on the colour with its alpha taken out and put back
const DECLARATIONS = `
uniform vec3 amethystora_row0;
uniform vec3 amethystora_row1;
uniform vec3 amethystora_row2;
vec3 amethystora_linear(vec3 c) {
    return mix(c / 12.92, pow((c + 0.055) / 1.055, vec3(2.4)), step(0.04045, c));
}
vec3 amethystora_encoded(vec3 c) {
    return mix(c * 12.92, 1.055 * pow(c, vec3(1.0 / 2.4)) - 0.055, step(0.0031308, c));
}`;
const CODE = `
float amethystora_a = cogl_color_out.a;
vec3 amethystora_c = amethystora_a > 0.0 ? cogl_color_out.rgb / amethystora_a : vec3(0.0);
amethystora_c = amethystora_linear(clamp(amethystora_c, 0.0, 1.0));
amethystora_c = clamp(vec3(dot(amethystora_row0, amethystora_c), dot(amethystora_row1, amethystora_c),
    dot(amethystora_row2, amethystora_c)), 0.0, 1.0);
cogl_color_out = vec4(amethystora_encoded(amethystora_c) * amethystora_a, amethystora_a);`;

const layoutOf = view => {
    const rect = new Mtk.Rectangle();
    view.get_layout(rect);
    return rect;
};

export const FilterEffect = GObject.registerClass(
class AmethystoraAccessFilterEffect extends Clutter.Effect {
    _init(failed) {
        super._init();
        this._matrix = filterMatrix('none', 0);
        // Per screen (or, for the suites' whole-stage paint, per size): its texture, the framebuffer drawn into it and
        // the pipeline that draws it back
        this._views = new Map();
        this._failed = failed;
        // The screens that cannot be filtered: the GPU could not hold or copy their picture
        this.failedViews = new Set();
        // The screens in HDR, by their place on the stage ("x,y"), left unfiltered
        this.hdr = new Set();
        // The screen being drawn now, from the stage's before-paint
        this.currentView = null;
    }

    setMatrix(matrix) {
        this._matrix = matrix;
        for (const target of this._views.values())
            this._setUniforms(target.pipeline);
        this.get_actor()?.queue_redraw();
    }

    // The textures go now (the framebuffer holds the texture and its depth and stencil buffers), and the screens are
    // tried again
    release() {
        for (const target of this._views.values())
            target.offscreen.run_dispose();
        this._views.clear();
        this.failedViews.clear();
    }

    _setUniforms(pipeline) {
        for (let row = 0; row < 3; row++) {
            const location = pipeline.get_uniform_location(`amethystora_row${row}`);
            if (location >= 0)
                pipeline.set_uniform_float(location, 3, 1, this._matrix.slice(row * 3, row * 3 + 3));
        }
    }

    // The texture for a screen, the size of the framebuffer the screen is drawn into
    _target(key, framebuffer) {
        const width = framebuffer.get_width();
        const height = framebuffer.get_height();
        const known = this._views.get(key);
        if (known?.width === width && known.height === height)
            return known;
        if (known) {
            known.offscreen.run_dispose();
            this._views.delete(key);
        }
        const context = framebuffer.get_context();
        const texture = Cogl.Texture2D.new_with_size(context, width, height);
        const offscreen = Cogl.Offscreen.new_with_texture(texture);
        // Throws where the texture cannot be had (larger than the GPU takes)
        try {
            offscreen.allocate();
        } catch (e) {
            offscreen.run_dispose();
            throw Object.assign(e, {kind: 'large'});
        }
        // And where the GPU cannot copy a framebuffer at all (some GLES 2 chips): one pixel copied once, here
        try {
            framebuffer.blit(offscreen, 0, 0, 0, 0, 1, 1);
        } catch (e) {
            offscreen.run_dispose();
            throw Object.assign(e, {kind: 'gpu'});
        }
        const pipeline = Cogl.Pipeline.new(context);
        pipeline.set_layer_texture(0, texture);
        // Pixel for pixel: the texture is the screen's own size
        pipeline.set_layer_filters(0, Cogl.PipelineFilter.NEAREST, Cogl.PipelineFilter.NEAREST);
        pipeline.add_snippet(Cogl.Snippet.new(Cogl.SnippetHook.FRAGMENT, DECLARATIONS, CODE));
        this._setUniforms(pipeline);
        const target = {width, height, offscreen, pipeline};
        this._views.set(key, target);
        return target;
    }

    vfunc_paint_node(node, paintContext, _flags) {
        const actor = this.get_actor();
        const framebuffer = paintContext.get_framebuffer();
        node.add_child(new Clutter.ActorNode(actor, -1));
        // The screen this paint is for: the one being drawn, when this paint draws into its framebuffer (GJS has no
        // get_stage_view; a paint for no screen, such as a screenshot made during a screen's paint, draws elsewhere)
        let view = this.currentView?.get_framebuffer() === framebuffer ? this.currentView : null;
        let layout = view ? layoutOf(view) : null;
        if (view && (this.failedViews.has(view) || this.hdr.has(`${layout.x},${layout.y}`)))
            return;
        // For the suites only: a whole-stage paint (a screenshot) drawn as the screen shows it
        if (!view && this.everyPaintForTest)
            view = `${framebuffer.get_width()}x${framebuffer.get_height()}`;
        if (!view)
            return;
        let target;
        try {
            target = this._target(view, framebuffer);
        } catch (e) {
            if (typeof view !== 'string')
                this.failedViews.add(view);
            this._failed?.(e);
            return;
        }
        // The whole screen is copied; drawing back reaches only the part being redrawn, as everything drawn does. (Not
        // less: the redraw clip's rectangles cannot be read from GJS without ending the shell.)
        const layer = Clutter.LayerNode.new_to_framebuffer(target.offscreen, target.pipeline);
        const blit = Clutter.BlitNode.new(framebuffer);
        blit.add_blit_rectangle(0, 0, 0, 0, target.width, target.height);
        layer.add_child(blit);
        let box;
        if (layout) {
            // Exactly the screen's place: the stage's projection carries the rounding of its viewport
            const scale = view.get_scale();
            box = {x1: layout.x, y1: layout.y, x2: layout.x + target.width / scale, y2: layout.y + target.height / scale};
        } else {
            const [x, y, width] = framebuffer.get_viewport4fv();
            const scale = width / global.stage.width;
            box = {x1: -x / scale, y1: -y / scale, x2: (target.width - x) / scale, y2: (target.height - y) / scale};
        }
        layer.add_texture_rectangle(new Clutter.ActorBox(box), 0, 0, 1, 1);
        node.add_child(layer);
    }

    get bytes() {
        // Colour, and the depth and stencil buffer Cogl gives each framebuffer
        return [...this._views.values()].reduce((sum, t) => sum + t.width * t.height * 8, 0);
    }
});

const FilterTile = GObject.registerClass(
class AmethystoraAccessFilterTile extends QuickMenuToggle {
    _init(filter) {
        super._init({title: _('Colour filter'), iconName: 'color-select-symbolic', toggleMode: true});
        this._amethystoraQuickId = 'colour-filter';
        this._filter = filter;
        this.menu.setHeader('color-select-symbolic', _('Colour filter'));

        this._kinds = new Map();
        for (const id of FILTERS) {
            const item = new PopupMenu.PopupMenuItem(filterName(id));
            item.connect('activate', () => this._filter.choose(id));
            this.menu.addMenuItem(item);
            this._kinds.set(id, item);
        }
        this.menu.addMenuItem(new PopupMenu.PopupSeparatorMenuItem(_('Strength')));
        this._strengths = new Map();
        for (const value of STRENGTHS) {
            const item = new PopupMenu.PopupMenuItem(strengthName(value));
            item.connect('activate', () => this._filter.setStrength(value));
            this.menu.addMenuItem(item);
            this._strengths.set(value, item);
        }
        this.connect('clicked', () => this._filter.setOn(this.checked));
    }

    sync(filter, on, strength) {
        this.checked = on;
        this.subtitle = filterName(filter);
        for (const [id, item] of this._kinds)
            item.setOrnament(id === filter ? PopupMenu.Ornament.CHECK : PopupMenu.Ornament.NONE);
        for (const [value, item] of this._strengths)
            item.setOrnament(value === strength ? PopupMenu.Ornament.CHECK : PopupMenu.Ornament.NONE);
    }
});

export function filterName(id) {
    switch (id) {
    case 'protan':
        return _('Red (protan)');
    case 'deutan':
        return _('Green (deutan)');
    case 'tritan':
        return _('Blue-yellow');
    case 'grey':
        return _('Grey');
    default:
        return _('Off');
    }
}

function strengthName(value) {
    switch (value) {
    case 50:
        return _('Half');
    case 75:
        return _('¾');
    default:
        return _('Full');
    }
}


// What could not be filtered, in the person's words
function failureText(kind) {
    switch (kind) {
    case 'hdr':
        return _('A screen in HDR is shown without the colour filter, which does not work with HDR yet.');
    case 'gpu':
        return _('This graphics chip cannot copy the screen, so the colour filter cannot be drawn.');
    default:
        return _('A screen is too large for the colour filter, and is shown without it.');
    }
}

export class Filter {
    constructor(settings) {
        this._settings = settings;
        this._effect = null;
        this._indicator = null;
        this._tile = null;
        this._unredirected = false;
        // Matrices other modules lay over the filter, by their name, applied after it in the order they came
        this._extras = new Map();
    }

    enable() {
        this._settings.connectObject(
            'changed::filter', () => this.sync(),
            'changed::filter-on', () => this.sync(),
            'changed::filter-strength', () => this.sync(),
            this);
        this.sync();
    }

    disable() {
        this._settings?.disconnectObject(this);
        this._extras.clear();
        this._setEffect(null);
        this._setTile(false);
        if (this._idle) {
            GLib.source_remove(this._idle);
            this._idle = 0;
        }
        this._hdrCancel?.cancel();
        this._hdrCancel = null;
    }

    // The tile's choices, written to the keys so that Control and `ame` see them
    choose(id) {
        this._settings.set_string('filter', id);
        this._settings.set_boolean('filter-on', true);
    }

    setOn(on) {
        this._settings.set_boolean('filter-on', on);
    }

    setStrength(value) {
        this._settings.set_int('filter-strength', value);
    }

    // Another module's matrix over the filter (row by row, 9 numbers), or null to take it away
    setExtra(id, matrix) {
        if (matrix?.length === 9 && matrix.every(Number.isFinite))
            this._extras.set(id, [...matrix]);
        else
            this._extras.delete(id);
        this.sync();
    }

    sync() {
        if (!this._settings)
            return;
        const filter = this._settings.get_string('filter');
        const chosen = FILTERS.includes(filter);
        const on = chosen && this._settings.get_boolean('filter-on');
        const strength = this._settings.get_int('filter-strength');
        let matrix = on ? filterMatrix(filter, strength / 100) : null;
        for (const extra of this._extras.values())
            matrix = matrix ? multiply(extra, matrix) : extra;
        this._setEffect(matrix);
        this._setTile(chosen);
        this._tile?.sync(filter, on, strength);
    }

    _setEffect(matrix) {
        if (matrix && !this._effect) {
            this._effect = new FilterEffect(e => this._failed(e.kind ?? 'large', e));
            this._effect.setMatrix(matrix);
            global.stage.add_effect_with_name(EFFECT_NAME, this._effect);
            // The screen each paint is for, and the screens in HDR
            global.stage.connectObject('before-paint', (stage, view) => {
                if (this._effect)
                    this._effect.currentView = view;
            }, this);
            Main.layoutManager.connectObject('monitors-changed', () => {
                this._effect?.release();
                this._readHdr();
            }, this);
            this._readHdr();
            global.compositor.disable_unredirect();
            this._unredirected = true;
        } else if (matrix) {
            this._effect.setMatrix(matrix);
        } else if (this._effect) {
            global.stage.disconnectObject(this);
            Main.layoutManager.disconnectObject(this);
            global.stage.remove_effect(this._effect);
            this._effect.release();
            this._effect.currentView = null;
            this._effect = null;
        }
        if (!this._effect && this._unredirected) {
            global.compositor.enable_unredirect();
            this._unredirected = false;
        }
    }

    // Which screens are in HDR, as Settings reads it: a monitor's color-mode in Mutter's display configuration
    _readHdr() {
        this._hdrCancel?.cancel();
        const cancel = new Gio.Cancellable();
        this._hdrCancel = cancel;
        Gio.DBus.session.call('org.gnome.Mutter.DisplayConfig', '/org/gnome/Mutter/DisplayConfig',
            'org.gnome.Mutter.DisplayConfig', 'GetCurrentState', null, null, Gio.DBusCallFlags.NONE, -1, cancel,
            (bus, result) => {
                let state;
                try {
                    state = bus.call_finish(result).deepUnpack();
                } catch {
                    return;
                }
                if (cancel.is_cancelled() || !this._effect)
                    return;
                const [, monitors, logical] = state;
                const hdr = new Set(monitors.filter(([, , properties]) => (properties['color-mode']?.unpack() ?? 0) !== 0)
                    .map(([[connector]]) => connector));
                this._effect.hdr = new Set(logical.filter(([, , , , , members]) =>
                    members.some(([connector]) => hdr.has(connector))).map(([x, y]) => `${x},${y}`));
                if (this._effect.hdr.size > 0)
                    this._failed('hdr', null);
                global.stage.queue_redraw();
            });
    }

    // A screen left unfiltered: said in the journal, and to the person once a session for each reason, after this paint
    _failed(kind, error) {
        console.warn(`Amethystora UI: the colour filter is not drawn on a screen (${kind})${error ? `: ${error.message}` : ''}`);
        if (told.has(kind) || this._idle)
            return;
        told.add(kind);
        this._idle = GLib.idle_add(GLib.PRIORITY_DEFAULT, () => {
            this._idle = 0;
            Main.notify(_('Colour filter'), failureText(kind));
            // A screen kept its last filtered picture where nothing was redrawn
            global.stage.queue_redraw();
            return GLib.SOURCE_REMOVE;
        });
    }

    _setTile(shown) {
        if (shown && !this._indicator) {
            this._indicator = new SystemIndicator();
            this._tile = new FilterTile(this);
            this._indicator.quickSettingsItems.push(this._tile);
            Main.panel.statusArea.quickSettings.addExternalIndicator(this._indicator);
        } else if (!shown && this._indicator) {
            // The tile's menu lives in quick settings' own grid, not under the tile: destroyed apart, as the battery's
            this._tile.menu.destroy();
            this._tile.destroy();
            this._tile = null;
            this._indicator.destroy();
            this._indicator = null;
        }
    }

    state() {
        const failedScreens = this._effect?.failedViews.size ?? 0;
        const hdrScreens = this._effect?.hdr.size ?? 0;
        return {
            effect: !!global.stage.get_effect(EFFECT_NAME),
            applied: !!this._effect && failedScreens === 0 && hdrScreens === 0,
            tile: !!this._tile,
            unredirectHeld: this._unredirected,
            bytes: this._effect?.bytes ?? 0,
            failedScreens,
            hdrScreens,
            extras: [...this._extras.keys()],
        };
    }
}
