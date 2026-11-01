// What an app's icon does while the app starts (spec 1.3), until its first new window, until it stops, or for six
// seconds at most: a ring that runs round the icon (Halo), a hop (Hop), a pulse (Breathe), or nothing. It plays on
// every dock item of the app, for a start from the dock and for one from anywhere else (the app grid, the search).
// GNOME's own zoom of the icon is not played: it would be a second sign, and "off" means none.

import GLib from 'gi://GLib';
import GObject from 'gi://GObject';
import Shell from 'gi://Shell';
import St from 'gi://St';

import {EASE, EDGES, Layer, REDUCED_FADE, scaleFactor, slowed} from './util.js';

const LONGEST = 6000;
const HALO = {fade: 210, turn: 700, width: 3, outside: 14};
// The board's 1.3 s hop: up 26, down, up 14, down, then still
const HOP = {segment: 234, still: 364, heights: [26, 0, 14, 0]};
const BREATHE = {segment: 350, scale: 0.9, opacity: 179};
const SETTLE = 120;

// The ring: clear for the first 55 % of the turn, rising to the ring's colour at 92 % and to the tip's at the end
export const Ring = GObject.registerClass(
class AmethystoraDockHalo extends St.DrawingArea {
    _init(size) {
        super._init({style_class: 'amethystora-dock-halo', width: size, height: size, opacity: 0});
        this._tip = new St.Widget({style_class: 'amethystora-dock-halo-tip', visible: false});
        this.add_child(this._tip);
        this.set_pivot_point(0.5, 0.5);
    }

    vfunc_repaint() {
        const cr = this.get_context();
        const [width, height] = this.get_surface_size();
        const ring = this.get_theme_node().get_foreground_color();
        const tip = this._tip.get_theme_node().get_foreground_color();
        const line = HALO.width * scaleFactor() * this.get_resource_scale();
        const radius = Math.min(width, height) / 2 - line / 2;
        cr.setLineWidth(line);
        const steps = 120;
        for (let i = 0; i < steps; i++) {
            const f = (i + 0.5) / steps;
            let colour = ring;
            let alpha = 0;
            if (f >= 0.92) {
                const t = (f - 0.92) / 0.08;
                colour = {
                    red: ring.red + (tip.red - ring.red) * t,
                    green: ring.green + (tip.green - ring.green) * t,
                    blue: ring.blue + (tip.blue - ring.blue) * t,
                    alpha: ring.alpha + (tip.alpha - ring.alpha) * t,
                };
                alpha = 1;
            } else if (f >= 0.55) {
                alpha = (f - 0.55) / 0.37;
            }
            if (alpha <= 0)
                continue;
            cr.setSourceRGBA(colour.red / 255, colour.green / 255, colour.blue / 255, colour.alpha / 255 * alpha);
            // From the top, clockwise, each arc a hair longer so that no seam shows between them
            const start = -Math.PI / 2 + i * 2 * Math.PI / steps;
            cr.arc(width / 2, height / 2, radius, start, start + 2 * Math.PI / steps + 0.004);
            cr.stroke();
        }
        cr.$dispose();
    }
});

export class Launch {
    constructor(dock) {
        this._dock = dock;
        this._runs = new Map();
        this.last = null;
        this._mode = 'off';
    }

    start(mode) {
        this._mode = mode;
        // An app that starts from anywhere, if the dock has it
        Shell.AppSystem.get_default().connectObject('app-state-changed', (system, app) => {
            if (app.state === Shell.AppState.STARTING && this._dock.itemsFor(app).length)
                this.begin(app);
        }, this);
    }

    stop() {
        Shell.AppSystem.get_default().disconnectObject(this);
        for (const run of [...this._runs.values()])
            this._end(run, false, true);
        this._runs.clear();
    }

    // A feedback under way ends; the next start shows the new one
    setMode(mode) {
        if (mode === this._mode)
            return;
        for (const run of [...this._runs.values()])
            this._end(run, false, true);
        this._mode = mode;
    }

    get rings() {
        return [...this._runs.values()].flatMap(run => run.shows.filter(show => show.ring));
    }

    // The app was started (by the dock, or seen starting)
    begin(app) {
        if (this._runs.has(app))
            return;
        const run = {
            app,
            windows: new Set(app.get_windows()),
            shows: [],
            record: {app: app.get_id(), started: GLib.get_monotonic_time(), firstWindow: 0, ended: 0},
        };
        this._runs.set(app, run);
        this.last = run.record;
        app.connectObject(
            'windows-changed', () => {
                const fresh = app.get_windows().some(window => !run.windows.has(window) && !window.skip_taskbar);
                if (fresh)
                    this._end(run, true);
            },
            'notify::state', () => {
                if (app.state === Shell.AppState.STOPPED)
                    this._end(run, false);
            }, run);
        run.timeout = GLib.timeout_add(GLib.PRIORITY_DEFAULT, LONGEST, () => {
            run.timeout = 0;
            this._end(run, false);
            return GLib.SOURCE_REMOVE;
        });
        if (this._mode !== 'off') {
            for (const state of this._dock.itemsFor(app))
                run.shows.push(this._show(state));
        }
    }

    // A dock was made again while an app starts: its item shows the feedback too
    adopt(state) {
        const run = this._runs.get(state.key);
        if (run && this._mode !== 'off' && !run.shows.some(show => show.state === state))
            run.shows.push(this._show(state));
    }

    _end(run, windowed, now = false) {
        if (this._runs.get(run.app) !== run)
            return;
        this._runs.delete(run.app);
        const time = GLib.get_monotonic_time();
        if (windowed)
            run.record.firstWindow = time;
        run.record.ended = time;
        run.app.disconnectObject(run);
        if (run.timeout)
            GLib.source_remove(run.timeout);
        run.timeout = 0;
        for (const show of run.shows)
            this._hide(show, now);
        run.shows = [];
    }

    // ---- What is drawn ----

    _show(state) {
        const level = this._dock.motionLevel();
        const button = state.button;
        const show = {state, level};
        const bin = button.icon?._iconBin;
        if (this._mode === 'halo') {
            const container = button._iconContainer;
            if (!container)
                return show;
            const size = Math.round((state.dash.iconSize + HALO.outside) * scaleFactor());
            const layer = new Layer();
            const ring = new Ring(size);
            layer.add_child(ring);
            container.add_child(layer);
            const place = () => ring.set_position(Math.round((layer.width - size) / 2),
                Math.round((layer.height - size) / 2));
            layer.connect('notify::allocation', place);
            place();
            show.ring = ring;
            show.layer = layer;
            // The dock's container can go first (a screen added or taken away): then there is nothing to hide
            layer.connect('destroy', () => {
                show.ring = null;
                show.layer = null;
            });
            if (level === 'none') {
                ring.opacity = 255;
            } else {
                ring.ease({
                    opacity: 255,
                    duration: level === 'reduced' ? REDUCED_FADE : HALO.fade,
                    mode: EASE.inOut,
                });
                if (level === 'full') {
                    ring.ease({
                        rotation_angle_z: 360,
                        duration: HALO.turn,
                        mode: EASE.linear,
                        repeatCount: -1,
                    });
                }
            }
        } else if (bin && level !== 'none' && (this._mode === 'hop' || this._mode === 'breathe')) {
            show.bin = bin;
            show.binDestroyed = bin.connect('destroy', () => {
                show.bin = null;
            });
            bin.set_pivot_point(0.5, 0.5);
            this._cycle(show, 0);
        }
        return show;
    }

    // One segment of Hop or Breathe after another, the cycle again while the app starts
    _cycle(show, step) {
        if (show.done || !show.bin)
            return;
        const {bin, state, level} = show;
        const edge = EDGES[state.dash.edgeName];
        const per = state.dash.iconSize / 48 * scaleFactor();
        const next = () => this._cycle(show, step + 1);
        if (level === 'reduced') {
            // Nothing travels: the picture dims and comes back
            const dim = step % 2 === 0;
            if (step % 4 === 2) {
                show.wait = GLib.timeout_add(GLib.PRIORITY_DEFAULT, slowed(HOP.still), () => {
                    show.wait = 0;
                    next();
                    return GLib.SOURCE_REMOVE;
                });
                return;
            }
            bin.ease({opacity: dim ? BREATHE.opacity : 255, duration: REDUCED_FADE, mode: EASE.out, onComplete: next});
            return;
        }
        if (this._mode === 'hop') {
            const at = step % 5;
            if (at === 4) {
                show.wait = GLib.timeout_add(GLib.PRIORITY_DEFAULT, slowed(HOP.still), () => {
                    show.wait = 0;
                    next();
                    return GLib.SOURCE_REMOVE;
                });
                return;
            }
            const value = edge.away * HOP.heights[at] * per;
            bin.ease({
                [edge.short === 'y' ? 'translation_y' : 'translation_x']: value,
                duration: HOP.segment,
                mode: EASE.out,
                onComplete: next,
            });
        } else {
            const low = step % 2 === 0;
            bin.ease({
                scale_x: low ? BREATHE.scale : 1,
                scale_y: low ? BREATHE.scale : 1,
                opacity: low ? BREATHE.opacity : 255,
                duration: BREATHE.segment,
                mode: EASE.sine,
                onComplete: next,
            });
        }
    }

    _hide(show, now) {
        show.done = true;
        if (show.wait)
            GLib.source_remove(show.wait);
        show.wait = 0;
        if (show.ring) {
            const {ring, layer} = show;
            if (now || show.level === 'none' || !ring.mapped) {
                layer.destroy();
                return;
            }
            ring.ease({
                opacity: 0,
                duration: show.level === 'reduced' ? REDUCED_FADE : HALO.fade,
                mode: EASE.inOut,
                onComplete: () => layer.destroy(),
            });
            return;
        }
        const bin = show.bin;
        if (!bin)
            return;
        bin.disconnect(show.binDestroyed);
        bin.remove_all_transitions();
        const rest = {translation_x: 0, translation_y: 0, scale_x: 1, scale_y: 1, opacity: 255};
        if (now)
            bin.set(rest);
        else
            bin.ease({...rest, duration: SETTLE, mode: EASE.out});
    }
}

// The ring's state for tests: the strongest ring of an item
export function ringOpacity(launch, state) {
    let most = 0;
    for (const show of launch.rings) {
        if (show.state === state && show.ring)
            most = Math.max(most, show.ring.opacity);
    }
    return most;
}

