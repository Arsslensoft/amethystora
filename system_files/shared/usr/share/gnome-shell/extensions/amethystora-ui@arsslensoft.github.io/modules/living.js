// Living wallpapers (R6-07, offered, off): a few wallpapers that move slowly, an aurora, drifting facets, rain on glass
// and a slow dusk, drawn in the theme's colours over GNOME's own wallpaper. They move only while they can be seen, and
// are a still picture otherwise: nothing is drawn for them while every screen is covered by a window that fills it
// (they are hidden then), and they hold still in the overview, on battery, in the power saver or a focus profile that
// asks for less (comfort's lighter), while a game is in front (Games), and with reduced motion or GNOME's animations
// off. While they move, each screen is drawn again FPS times a second: one full-screen pass of a small shader
// (living/living.glsl), from a Clutter.Effect with its own Cogl pipeline (glass/shader.js), never Shell.GLSLEffect,
// which GNOME 51 no longer has.
//
// A workspace with a picture of its own (the tiling's) shows that picture over this, as it shows it over GNOME's
// wallpaper. The glass is cut from GNOME's wallpaper, which stays as it is under this.
//
// Key (org.gnome.shell.extensions.amethystora-ui.living, ame desktop living-wallpaper, Control):
//   wallpaper  'off'   off, aurora, facets, rain or dusk
// Its colours are the theme's (themed/shell-living.css.tpl), read from the layer's style.

import Clutter from 'gi://Clutter';
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import GObject from 'gi://GObject';
import Meta from 'gi://Meta';
import St from 'gi://St';

import * as Main from 'resource:///org/gnome/shell/ui/main.js';

import {Shader} from './glass/shader.js';

const SCHEMA = 'org.gnome.shell.extensions.amethystora-ui.living';
export const KINDS = ['aurora', 'facets', 'rain', 'dusk'];
// Frames a second while a living wallpaper moves: slow pictures need no more
const FPS = 15;
// How slowly the pictures move (seconds of picture time to a second)
const PACE = 1;
// Neutral stand-ins until a theme's stylesheet says otherwise
const STAND_IN = {ground: [0.11, 0.07, 0.17], accent: [0.57, 0.28, 0.94], second: [0.36, 0.55, 0.96]};

const UPOWER = 'org.freedesktop.UPower';
const UPOWER_PATH = '/org/freedesktop/UPower';

const LivingEffect = GObject.registerClass(
class AmethystoraLivingEffect extends Clutter.Effect {
    _init(living) {
        super._init();
        this._living = living;
        this._pipeline = null;
    }

    vfunc_paint_node(node, paintContext, _flags) {
        const actor = this.get_actor();
        const living = this._living;
        if (!actor || !living.shader || actor.width < 1 || actor.height < 1)
            return;
        const shader = living.shader;
        if (!this._pipeline) {
            this._pipeline = shader.pipeline(paintContext.get_framebuffer().get_context());
            shader.set(this._pipeline, 'u_kind', living.kindIndex);
        }
        const colors = living.colorsOf(actor);
        const pipeline = this._pipeline;
        shader.set(pipeline, 'u_time', living.time);
        shader.set(pipeline, 'u_size', actor.width, actor.height);
        shader.set(pipeline, 'u_kind', living.kindIndex);
        shader.set(pipeline, 'u_ground', ...colors.ground);
        shader.set(pipeline, 'u_accent', ...colors.accent);
        shader.set(pipeline, 'u_second', ...colors.second);
        const pipelineNode = new Clutter.PipelineNode(pipeline);
        node.add_child(pipelineNode);
        pipelineNode.add_texture_rectangle(new Clutter.ActorBox({x1: 0, y1: 0, x2: actor.width, y2: actor.height}),
            0, 0, 1, 1);
        living.frames++;
    }
});

export class Living {
    constructor(ext) {
        this._ext = ext;
    }

    enable() {
        this.frames = 0;
        this._settings = this._ext.getSettings(SCHEMA);
        this._settings.connectObject('changed::wallpaper', () => this._sync(), this);
        this._sync();
    }

    disable() {
        this._stop();
        this._settings?.disconnectObject(this);
        // Disposed now rather than by the collector (dock.js says why)
        this._settings?.run_dispose();
        this._settings = null;
    }

    get kind() {
        const kind = this._settings?.get_string('wallpaper') ?? 'off';
        return KINDS.includes(kind) ? kind : 'off';
    }

    get kindIndex() {
        return Math.max(0, KINDS.indexOf(this.kind));
    }

    get time() {
        return ((GLib.get_monotonic_time() - (this._started ?? 0)) / 1e6 * PACE) % 10000;
    }

    _sync() {
        if (this.kind === 'off') {
            this._stop();
            return;
        }
        if (!this._layers)
            this._start();
        for (const layer of this._layers ?? [])
            layer.queue_redraw();
        this._update();
    }

    // ---- Built while on ----

    _start() {
        const file = this._ext.dir.get_child('modules').get_child('living').get_child('living.glsl');
        const [, bytes] = file.load_contents(null);
        this.shader = new Shader(new TextDecoder().decode(bytes), 'cogl_color_out = living_color();', 1);
        this._started = GLib.get_monotonic_time();
        this._build();

        const queue = () => this._queue();
        global.display.connectObject(
            'restacked', queue,
            'in-fullscreen-changed', queue,
            'grab-op-end', queue,
            'window-created', queue, this);
        global.window_manager.connectObject(
            'size-changed', queue,
            'minimize', queue,
            'unminimize', queue,
            'map', queue,
            'destroy', queue, this);
        global.workspace_manager.connectObject('active-workspace-changed', queue, this);
        Main.overview.connectObject('showing', queue, 'hidden', queue, this);
        Main.layoutManager.connectObject('monitors-changed', () => {
            this._destroyLayers();
            this._build();
            this._queue();
        }, this);
        this._unlistenComfort = this._ext.modules?.get('comfort')?.listen?.(queue) ?? null;
        this._upowerCancellable = new Gio.Cancellable();
        Gio.DBusProxy.new_for_bus(Gio.BusType.SYSTEM, Gio.DBusProxyFlags.NONE, null, UPOWER, UPOWER_PATH, UPOWER,
            this._upowerCancellable, (source, result) => {
                try {
                    this._upower = Gio.DBusProxy.new_for_bus_finish(result);
                } catch {
                    return;
                }
                if (!this._layers) {
                    this._upower = null;
                    return;
                }
                this._upower.connectObject('g-properties-changed', queue, this);
                this._queue();
            });
    }

    // One layer for each screen, over GNOME's wallpaper of it, under the windows and a workspace's own picture
    _build() {
        this._layers = [];
        const group = Main.layoutManager._backgroundGroup;
        for (const monitor of Main.layoutManager.monitors) {
            const layer = new St.Widget({
                style_class: 'amethystora-living',
                reactive: false,
                x: monitor.x, y: monitor.y, width: monitor.width, height: monitor.height,
            });
            layer._amethystoraMonitor = monitor.index;
            layer.add_effect(new LivingEffect(this));
            layer.connect('style-changed', () => {
                layer._amethystoraColors = null;
                layer.queue_redraw();
            });
            group.add_child(layer);
            this._layers.push(layer);
        }
    }

    _destroyLayers() {
        for (const layer of this._layers ?? [])
            layer.destroy();
        this._layers = null;
    }

    _stop() {
        this._stopTimer();
        if (this._later)
            global.compositor.get_laters().remove(this._later);
        this._later = 0;
        global.display.disconnectObject(this);
        global.window_manager.disconnectObject(this);
        global.workspace_manager.disconnectObject(this);
        Main.overview.disconnectObject(this);
        Main.layoutManager.disconnectObject(this);
        this._unlistenComfort?.();
        this._unlistenComfort = null;
        this._upowerCancellable?.cancel();
        this._upowerCancellable = null;
        this._upower?.disconnectObject(this);
        this._upower = null;
        this._destroyLayers();
        this.shader?.destroy();
        this.shader = null;
        this._why = null;
    }

    // The theme's colours, from the layer's stylesheet (shell-living.css), read once per style
    colorsOf(layer) {
        if (layer._amethystoraColors)
            return layer._amethystoraColors;
        const node = layer.get_theme_node();
        const colors = {...STAND_IN};
        for (const name of ['ground', 'accent', 'second']) {
            const [found, color] = node.lookup_color(`-amethystora-living-${name}`, false);
            if (found)
                colors[name] = [color.red / 255, color.green / 255, color.blue / 255];
        }
        layer._amethystoraColors = colors;
        return colors;
    }

    // ---- Moving only while seen ----

    _queue() {
        if (this._later || !this._layers)
            return;
        this._later = global.compositor.get_laters().add(Meta.LaterType.BEFORE_REDRAW, () => {
            this._later = 0;
            this._update();
            return false;
        });
    }

    // Whether a window fills the screen's work area on the workspace shown (or is fullscreen there)
    _covered(index) {
        if (global.display.get_monitor_in_fullscreen(index))
            return true;
        const area = Main.layoutManager.getWorkAreaForMonitor(index);
        const workspace = global.workspace_manager.get_active_workspace();
        return global.display.list_all_windows().some(window => {
            if (window.minimized || !window.showing_on_its_workspace?.() || window.get_monitor() !== index)
                return false;
            if (!window.located_on_workspace(workspace) || window.get_window_type() !== Meta.WindowType.NORMAL)
                return false;
            const rect = window.get_frame_rect();
            return rect.x <= area.x && rect.y <= area.y &&
                rect.x + rect.width >= area.x + area.width && rect.y + rect.height >= area.y + area.height;
        });
    }

    // Why it holds still now, or null while it may move
    _stillReason() {
        const comfort = this._ext.modules?.get('comfort');
        if (comfort?.reduceMotion || !St.Settings.get().enable_animations)
            return 'reduce-motion';
        if (comfort?.lighter)
            return 'power-saver';
        const battery = this._upower?.get_cached_property('OnBattery');
        if (battery?.get_type_string() === 'b' && battery.unpack())
            return 'battery';
        if (this._ext.modules?.get('focus')?.state?.()?.profile === 'games')
            return 'games';
        if (Main.overview.visible)
            return 'overview';
        return null;
    }

    _update() {
        if (!this._layers)
            return;
        let seen = 0;
        for (const layer of this._layers) {
            const covered = this._covered(layer._amethystoraMonitor);
            layer.visible = !covered;
            if (!covered)
                seen++;
        }
        const still = seen === 0 ? 'covered' : this._stillReason();
        this._why = still;
        if (still === null)
            this._startTimer();
        else
            this._stopTimer();
    }

    _startTimer() {
        if (this._timer)
            return;
        this._timer = GLib.timeout_add(GLib.PRIORITY_DEFAULT_IDLE, Math.round(1000 / FPS), () => {
            for (const layer of this._layers ?? []) {
                if (layer.visible)
                    layer.queue_redraw();
            }
            return GLib.SOURCE_CONTINUE;
        });
    }

    _stopTimer() {
        if (this._timer)
            GLib.source_remove(this._timer);
        this._timer = 0;
    }

    // For the suites and the harness
    state() {
        return {
            wallpaper: this.kind,
            layers: this._layers?.length ?? 0,
            shown: (this._layers ?? []).filter(layer => layer.visible).length,
            moving: !!this._timer,
            still: this._layers ? this._why ?? null : null,
            frames: this.frames,
        };
    }
}
