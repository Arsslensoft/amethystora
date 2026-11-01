// Amethystora Transitions: a new wallpaper or theme is revealed with an animation, the way swww draws
// it on Hyprland, instead of the wallpaper crossfading while the rest of the desktop repaints piece by
// piece.
//
// The moment a change begins, the screen is frozen as a picture of itself, in GNOME Shell's own
// screenTransition actor (the one the Dark Style toggle fades out). While that actor is showing, the
// background manager swaps the wallpaper at once instead of fading it, so the whole change happens
// unseen underneath. Once nothing has changed for SETTLE_MS, a shader opens the picture onto the new
// desktop.
//
// A change begins in one of three ways:
//   - amethystora-theme calls Hold on /org/amethystora/Transitions before it writes anything and
//     Release once it is done, so a theme is one transition however long it takes to apply
//   - a wallpaper or appearance key changes from anywhere else: Settings, gsettings, Super+Ctrl+Space
//   - the Dark Style toggle in the quick settings, through screenTransition.run()
// Whatever happens, the screen is never held for more than MAX_HOLD_MS.
//
// At login, reveal.js plays the boot splash's gem into the top bar as the desktop first appears.

import Clutter from 'gi://Clutter';
import Cogl from 'gi://Cogl';
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import GObject from 'gi://GObject';
import Graphene from 'gi://Graphene';
import Mtk from 'gi://Mtk';
import St from 'gi://St';

import * as Config from 'resource:///org/gnome/shell/misc/config.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import {Extension, InjectionManager} from 'resource:///org/gnome/shell/extensions/extension.js';

import {Reveal} from './reveal.js';

const GNOME_MAJOR = parseInt(Config.PACKAGE_VERSION, 10);

// How long the desktop has to stay unchanged before it is revealed: long enough for the apps to
// repaint in the new theme, and for a burst of settings to count as one change
const SETTLE_MS = 300;
const MAX_HOLD_MS = 5000;

// swww's default curve: a slow start, then the new desktop sweeps in
const BEZIER = [new Graphene.Point({x: 0.54, y: 0}), new Graphene.Point({x: 0.34, y: 0.99})];

// In the order the shader numbers them. random picks one of the first four.
const STYLES = ['grow', 'outer', 'wipe', 'wave', 'fade'];

const EFFECT_NAME = 'amethystora-transition';

// The keys whose change shows across the whole screen
const WATCHED = {
    'org.gnome.desktop.background': ['picture-uri', 'picture-uri-dark', 'picture-options', 'primary-color'],
    'org.gnome.desktop.interface': ['color-scheme', 'accent-color', 'gtk-theme', 'icon-theme'],
};

const DBUS_INTERFACE = `
<node>
  <interface name="org.amethystora.Transitions">
    <method name="Hold"/>
    <method name="Release"/>
  </interface>
</node>`;

// The frozen picture is drawn where the new desktop has not reached yet. d is how far a pixel is ahead
// of the edge, in logical pixels: from soft up the old desktop is whole, from 0 down the new one shows,
// and in between the two blend. A band of the accent colour glows along the edge while it moves. The
// effect's pipeline blends premultiplied, as Cogl does by default, so the colour leaves already
// multiplied by the alpha.
const DECLARATIONS = `
uniform sampler2D picture;
uniform float progress;
uniform float style;
uniform vec2 resolution;
uniform vec2 origin;
uniform vec2 direction;
uniform vec3 glow;

float reach(vec2 from) {
    return max(max(length(from), length(from - vec2(resolution.x, 0.0))),
               max(length(from - vec2(0.0, resolution.y)), length(from - resolution)));
}
`;

const CODE = `
vec3 frozen = texture2D(picture, cogl_tex_coord_in[0].st).rgb;
vec2 p = cogl_tex_coord_in[0].st * resolution;
float soft = max(resolution.x, resolution.y) * 0.03;
float keep;
float shine = 0.0;

if (style > 3.5) {
    keep = 1.0 - progress;
} else {
    float d;
    if (style < 0.5) {
        d = length(p - origin) + soft - progress * (reach(origin) + soft);
    } else if (style < 1.5) {
        d = reach(origin) - length(p - origin) + soft - progress * (reach(origin) + soft);
    } else {
        float c1 = dot(vec2(resolution.x, 0.0), direction);
        float c2 = dot(vec2(0.0, resolution.y), direction);
        float c3 = dot(resolution, direction);
        float lo = min(min(0.0, c1), min(c2, c3));
        float hi = max(max(0.0, c1), max(c2, c3));
        float amplitude = style > 2.5 ? soft * 1.5 : 0.0;
        float across = dot(p, vec2(-direction.y, direction.x)) / max(resolution.x, resolution.y);
        float wave = amplitude * (1.0 + sin(across * 31.4159));
        d = dot(p, direction) - lo + wave + soft - progress * (hi - lo + 2.0 * amplitude + soft);
    }
    keep = smoothstep(0.0, soft, d);
    shine = (1.0 - smoothstep(0.0, soft * 1.5, abs(d - soft * 0.5))) * 0.45 * sin(progress * 3.14159);
}

float alpha = keep + shine * (1.0 - keep);
cogl_color_out = vec4(frozen * keep * (1.0 - shine) + glow * shine, alpha);
`;

// Draws the actor's frozen picture, content of the screenTransition, through the shader above, in place of the
// actor's own paint. A plain Clutter.Effect has no offscreen pass to hand the picture over, so the pipeline
// samples the content's texture itself. (Shell.GLSLEffect, which did that, is gone from GNOME 51.)
const RevealEffect = GObject.registerClass(
class RevealEffect extends Clutter.Effect {
    _init() {
        super._init();
        this._uniforms = {};
        this._locations = new Map();
        this._context = null;
        this._pipeline = null;
        this._texture = null;
    }

    setUniforms(uniforms) {
        Object.assign(this._uniforms, uniforms);
        this.get_actor()?.queue_redraw();
    }

    // Lets go of the compiled program and the picture, for an effect taken off its actor
    forget() {
        this._context = null;
        this._pipeline = null;
        this._texture = null;
        this._locations.clear();
    }

    _pipelineFor(context) {
        if (this._context !== context) {
            this.forget();
            const pipeline = Cogl.Pipeline.new(context);
            pipeline.set_layer_null_texture(0);
            pipeline.set_layer_filters(0, Cogl.PipelineFilter.LINEAR, Cogl.PipelineFilter.LINEAR);
            pipeline.set_layer_wrap_mode(0, Cogl.PipelineWrapMode.CLAMP_TO_EDGE);
            pipeline.add_snippet(Cogl.Snippet.new(Cogl.SnippetHook.FRAGMENT, DECLARATIONS, CODE));
            pipeline.set_uniform_1i(pipeline.get_uniform_location('picture'), 0);
            this._pipeline = pipeline;
            this._context = context;
        }
        return this._pipeline;
    }

    vfunc_paint_node(node, paintContext, _flags) {
        const actor = this.get_actor();
        const texture = actor.content?.get_texture?.();
        if (!texture) {
            node.add_child(new Clutter.ActorNode(actor, -1));
            return;
        }

        const pipeline = this._pipelineFor(paintContext.get_framebuffer().get_context());
        if (this._texture !== texture) {
            pipeline.set_layer_texture(0, texture);
            this._texture = texture;
        }
        for (const [name, value] of Object.entries(this._uniforms)) {
            let location = this._locations.get(name);
            if (location === undefined) {
                location = pipeline.get_uniform_location(name);
                this._locations.set(name, location);
            }
            if (location < 0)
                continue;
            const values = [value].flat();
            if (values.length === 1)
                pipeline.set_uniform_1f(location, values[0]);
            else
                pipeline.set_uniform_float(location, values.length, 1, values);
        }

        const pipelineNode = new Clutter.PipelineNode(pipeline);
        node.add_child(pipelineNode);
        pipelineNode.add_texture_rectangle(
            new Clutter.ActorBox({x1: 0, y1: 0, x2: actor.width, y2: actor.height}), 0, 0, 1, 1);
    }
});

export default class TransitionsExtension extends Extension {
    enable() {
        this._settings = this.getSettings();
        this._actor = Main.layoutManager.screenTransition;
        this._effect = new RevealEffect();
        this._frozen = false;
        this._held = false;

        // GSettings only reports a change to a key that has been read since its handler was connected
        this._watched = Object.entries(WATCHED).map(([schema, keys]) => {
            const settings = new Gio.Settings({schema_id: schema});
            for (const key of keys) {
                settings.connectObject(`changed::${key}`, () => this._changed(), this);
                settings.get_value(key);
            }
            return settings;
        });

        const extension = this;
        this._injections = new InjectionManager();
        this._injections.overrideMethod(Object.getPrototypeOf(this._actor), 'run', run => function () {
            if (extension._freeze())
                extension._scheduleReveal();
            else
                run.call(this);
        });

        this._dbus = Gio.DBusExportedObject.wrapJSObject(DBUS_INTERFACE, {
            Hold: () => this._hold(),
            Release: () => this._release(),
        });
        this._dbus.export(Gio.DBus.session, '/org/amethystora/Transitions');

        // GNOME shows the other screens' wallpapers when it has started, and a wallpaper still loading then (a theme
        // applied during the start: an account's first login) would come in hidden, for the rest of the session
        if (Main.layoutManager._startingUp) {
            Main.layoutManager.connectObject('startup-complete', () => {
                for (const manager of Main.layoutManager._bgManagers ?? [])
                    manager._newBackgroundActor?.show();
            }, this);
        }

        // The reveal at login, which only happens while GNOME is starting
        this._loginReveal = new Reveal(this._settings);
        this._loginReveal.start();
    }

    disable() {
        // The top bar is never left hidden or out of place
        this._loginReveal?.stop();
        this._loginReveal = null;
        Main.layoutManager.disconnectObject(this);
        this._dbus.unexport();
        this._injections.clear();
        for (const settings of this._watched)
            settings.disconnectObject(this);
        if (this._frozen || this._timeline)
            this._finish();
        this._effect.forget();
        // Disposed here, on the main thread, rather than left to the collector: GJS sweeping a settings object while
        // dconf's worker dispatches a change to it deadlocks the shell (its toggle queue against the backend's lock)
        for (const settings of this._watched)
            settings.run_dispose();
        this._settings?.run_dispose();

        this._dbus = null;
        this._injections = null;
        this._watched = null;
        this._effect = null;
        this._actor = null;
        this._settings = null;
    }

    _changed() {
        if (this._freeze())
            this._scheduleReveal();
    }

    _hold() {
        if (this._freeze())
            this._held = true;
    }

    _release() {
        this._held = false;
        if (this._frozen)
            this._scheduleReveal();
    }

    // Freeze the screen as it is now. Returns false when there is to be no animation, and the change
    // then shows the way it would without this extension.
    _freeze() {
        if (this._frozen)
            return true;
        if (this._settings.get_string('style') === 'none' || !St.Settings.get().enable_animations)
            return false;

        // Mid-reveal, this takes the half-revealed screen, so a change that comes quickly after another
        // carries on from what is showing rather than jumping back
        const rect = new Mtk.Rectangle({x: 0, y: 0, width: global.screen_width, height: global.screen_height});
        let content;
        try {
            const [, , , scale] = global.stage.get_capture_final_size(rect);
            content = GNOME_MAJOR >= 50
                ? global.stage.paint_to_content(rect, scale, null, Clutter.PaintFlag.NO_CURSORS)
                : global.stage.paint_to_content(rect, scale, Clutter.PaintFlag.NO_CURSORS);
        } catch (error) {
            console.error(`Amethystora Transitions: could not capture the screen: ${error.message}`);
            return false;
        }

        // At progress 0, grow keeps the whole picture, whatever the style is to be
        const [x, y] = global.get_pointer();
        this._stopTimeline();
        this._effect.setUniforms({
            progress: 0,
            style: 0,
            resolution: [global.screen_width, global.screen_height],
            origin: [x, y],
        });
        Main.uiGroup.set_child_above_sibling(this._actor, null);
        this._actor.remove_all_transitions();
        this._actor.set({content, opacity: 255});
        if (!this._actor.get_effect(EFFECT_NAME))
            this._actor.add_effect_with_name(EFFECT_NAME, this._effect);
        this._actor.show();
        this._frozen = true;

        this._clearTimeout('_capId');
        this._capId = GLib.timeout_add(GLib.PRIORITY_DEFAULT, MAX_HOLD_MS, () => {
            this._capId = 0;
            // The screen is given back whatever goes wrong: a reveal that throws must not leave the old picture over
            // the desktop, where it looks like a machine that has hung
            try {
                this._reveal();
            } catch (error) {
                console.error(`Amethystora Transitions: the reveal failed: ${error.message}`);
                this._finish();
            }
            return GLib.SOURCE_REMOVE;
        });
        return true;
    }

    _scheduleReveal(delay = SETTLE_MS) {
        this._clearTimeout('_settleId');
        this._settleId = GLib.timeout_add(GLib.PRIORITY_DEFAULT, delay, () => {
            this._settleId = 0;
            if (this._held)
                return GLib.SOURCE_REMOVE;
            if (this._backgroundLoading())
                this._scheduleReveal(50);
            else
                this._reveal();
            return GLib.SOURCE_REMOVE;
        });
    }

    // A new wallpaper loads in its own time, and revealed before it is in, the old one would show
    // through. The background manager's fields are private, so they are read defensively: without
    // them, the reveal does not wait.
    _backgroundLoading() {
        return Main.layoutManager._bgManagers?.some(manager => manager._newBackgroundActor) ?? false;
    }

    _reveal() {
        this._clearTimeout('_settleId');
        this._clearTimeout('_capId');
        this._held = false;
        if (!this._frozen)
            return;
        this._frozen = false;

        let style = this._settings.get_string('style');
        let angle = Math.PI / 6;
        if (style === 'random') {
            style = STYLES[Math.floor(Math.random() * 4)];
            angle = Math.random() * 2 * Math.PI;
        }
        if (!STYLES.includes(style)) {
            this._finish();
            return;
        }

        this._effect.setUniforms({
            style: STYLES.indexOf(style),
            direction: [Math.cos(angle), Math.sin(angle)],
            glow: this._accent(),
        });

        this._timeline = new Clutter.Timeline({
            actor: this._actor,
            duration: this._settings.get_uint('duration'),
        });
        this._timeline.set_cubic_bezier_progress(...BEZIER);
        this._timeline.connectObject(
            'new-frame', () => this._effect.setUniforms({progress: this._timeline.get_progress()}),
            'stopped', () => this._finish(),
            this);
        this._timeline.start();
    }

    _finish() {
        this._clearTimeout('_settleId');
        this._clearTimeout('_capId');
        this._stopTimeline();
        this._frozen = false;
        this._held = false;
        if (this._actor.get_effect(EFFECT_NAME))
            this._actor.remove_effect(this._effect);
        this._effect.forget();
        this._actor.hide();
    }

    _stopTimeline() {
        if (!this._timeline)
            return;
        this._timeline.disconnectObject(this);
        this._timeline.stop();
        this._timeline = null;
    }

    _clearTimeout(name) {
        if (this[name])
            GLib.source_remove(this[name]);
        this[name] = 0;
    }

    // The edge glows in the accent colour of the theme being revealed
    _accent() {
        try {
            const [color] = St.ThemeContext.get_for_stage(global.stage).get_accent_color();
            return [color.red / 255, color.green / 255, color.blue / 255];
        } catch {
            return [0.57, 0.25, 0.67];
        }
    }
}
