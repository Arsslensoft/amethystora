// How windows go into the dock and come back, and how one workspace gives way to the next. Minimize a window and it
// gathers into a drop of glass in the theme's accent, which arcs down into the window's icon in the dock while a ring
// ripples out of the icon; bring it back, or open an app from the dock, and a drop rises from the icon and swells
// into the window, a little past its size and back. A click part way turns a window round from where it is. Changing
// workspace has depth: the windows travel a whole screen, the wallpaper a third of the way, and the windows step back
// a little as they go. With Reduce motion on, windows fade in place and workspaces crossfade; with GNOME's animations
// off, nothing moves at all, as GNOME makes it.
//
// A window can also arrive and leave its own way (the `open-close` key: pop, slide or fade, motion/arrive.js), in place of
// GNOME's own effect for a plain window.
//
// GNOME's window manager keeps its own handlers for minimize, unminimize, map and destroy; while this module draws
// those, it blocks them (and only them) and calls GNOME's own methods for everything it leaves to GNOME, and GNOME's own
// endings when its flights are over, so that the window always ends exactly as GNOME leaves it. Nothing is replaced
// on GNOME's objects. Nothing runs while nothing moves.
//
// For the other modules: flying(actor) is true while a flight runs on a window's actor (the glass leaves such a
// window without its pane), and launched(app) is the dock saying it has just opened an app.

import Clutter from 'gi://Clutter';
import GLib from 'gi://GLib';
import GObject from 'gi://GObject';
import Meta from 'gi://Meta';
import Shell from 'gi://Shell';
import St from 'gi://St';

import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import * as WorkspaceAnimation from 'resource:///org/gnome/shell/ui/workspaceAnimation.js';

import {Arrivals, STYLES as ARRIVALS} from './motion/arrive.js';
import {Depth} from './motion/depth.js';
import {dropletShader} from './motion/droplet.js';
import {Flight, frameCentre, scaleFactor} from './motion/flight.js';
import {OUT, RIPPLE_IN_MS, RIPPLE_OUT_MS, turnRound} from './motion/path.js';
import {Rise} from './motion/rise.js';

const SCHEMA = 'org.gnome.shell.extensions.amethystora-ui.motion';
// GNOME's handlers this module stands in for: the droplet's, and the opening's and closing's
const DROPLET = ['minimize', 'unminimize'];
// The kinds of window GNOME animates when they are minimized or come back
const TYPES = [Meta.WindowType.NORMAL, Meta.WindowType.MODAL_DIALOG, Meta.WindowType.DIALOG];
// How long after a click in the dock the app's first window still grows out of its icon, in microseconds
const LAUNCH_WINDOW = 10 * GLib.USEC_PER_SEC;
// The corners of a window when the desktop style says nothing, and the stand-ins for the theme's colours
const RADIUS = 12;
const ACCENT = [0x91 / 255, 0x48 / 255, 0xf0 / 255];
// Where the drop goes when the window has no icon: the bottom centre of its screen, this far up, in the size of an icon
const NO_ICON_LIFT = 32;
const NO_ICON_SIZE = 48;

export class Motion {
    constructor(ext) {
        this._ext = ext;
    }

    enable() {
        this._flights = new Map();
        // The windows of a saved set rising into their places (workspaces/sets.js), and how many have
        this._risings = new Map();
        this.risen = 0;
        this._resume = new Map();
        this._resumeLater = 0;
        this._blocked = new Map();
        this._drivers = new Set();
        this._ripples = new Set();
        this._hookId = 0;
        this._launch = null;
        this._shader = dropletShader();
        this._arrivals = new Arrivals((actor, t0) => this._tilingRest(actor, actor.meta_window?.get_frame_rect(), t0));
        this._settings = this._ext.getSettings(SCHEMA);
        this._settings.connectObject(
            'changed::minimize', () => this._sync(),
            'changed::workspace-switch', () => this._sync(),
            'changed::open-close', () => this._sync(),
            this);
        global.window_manager.connectObject(
            'minimize', (wm, actor) => this._minimize(wm, actor),
            'unminimize', (wm, actor) => this._unminimize(wm, actor),
            'map', (wm, actor) => this._map(wm, actor),
            'destroy', (wm, actor) => this._destroy(wm, actor),
            'kill-window-effects', (wm, actor) => this._kill(wm, actor),
            this);
        this._unlisten = this._comfort?.listen?.(() => this._sync()) ?? null;
        this._sync();
    }

    disable() {
        this._unlisten?.();
        this._unlisten = null;
        this._settings?.disconnectObject(this);
        // Disposed now rather than by the collector (dock.js says why)
        this._settings?.run_dispose();
        this._settings = null;
        global.window_manager.disconnectObject(this);
        // A flight under way ends where it was going, so that the window manager hears the end of every effect
        for (const flight of [...this._flights?.values() ?? []]) {
            try {
                this._end(flight);
            } catch (e) {
                console.error(`Amethystora UI: motion: a flight did not end cleanly: ${e.message}`);
            }
        }
        this._flights = null;
        this._arrivals?.stop();
        this._arrivals = null;
        for (const rise of [...this._risings?.values() ?? []]) {
            try {
                this._endRise(rise);
            } catch (e) {
                console.error(`Amethystora UI: motion: a rise did not end cleanly: ${e.message}`);
            }
        }
        this._risings = null;
        this._resume = null;
        if (this._resumeLater)
            global.compositor.get_laters().remove(this._resumeLater);
        this._resumeLater = 0;
        const ripples = [...this._ripples ?? []];
        this._ripples = null;
        for (const ripple of ripples)
            ripple.destroy();
        this._unhook();
        for (const driver of this._drivers ?? [])
            driver.stop(true);
        this._drivers = null;
        this._unblock();
        this._blocked = null;
        this._shader?.destroy();
        this._shader = null;
        this._launch = null;
    }

    get _comfort() {
        return this._ext.modules?.get('comfort') ?? null;
    }

    // Reduce motion, as the comfort module has it; GNOME's animations being off comes first wherever it is asked
    get _reduced() {
        return this._comfort?.reduceMotion ?? false;
    }

    // ---- For the other modules and the tests ----

    flying(actor) {
        return !!actor && (!!this._flights?.has(actor) || !!this._risings?.has(actor));
    }

    // The flight on a window's actor, for the tests to pause and step
    flight(actor) {
        return this._flights?.get(actor) ?? null;
    }

    // The dock opened an app: its first window to appear grows out of the app's icon
    launched(app) {
        if (!app || !this._flights)
            return;
        this._launch = {app, at: GLib.get_monotonic_time()};
    }

    // The window and the motion it is arriving or leaving with (open-close), for the tests
    arriving(actor) {
        return this._arrivals?.moving(actor) ?? false;
    }

    // ---- Which of GNOME's handlers are standing aside ----

    // pop, slide or fade while the key says one of them, else null (GNOME's own)
    get _arrival() {
        const style = this._settings?.get_string('open-close');
        return ARRIVALS.includes(style) ? style : null;
    }

    _sync() {
        if (!this._settings)
            return;
        const reduced = this._reduced;
        const droplet = this._settings.get_string('minimize') === 'droplet' || reduced;
        const arrival = this._arrival !== null;
        // The droplet's own paths of a window mapping (a set rising, an app the dock opened) only while it is chosen
        this._droplet = droplet;
        this._blockOnly({
            minimize: droplet, unminimize: droplet, map: droplet || arrival, destroy: arrival,
        });
        if (this._settings.get_string('workspace-switch') === 'depth' || reduced)
            this._hook();
        else
            this._unhook();
    }

    // Each of GNOME's handlers blocked or not, as wanted
    _blockOnly(wanted) {
        for (const [name, on] of Object.entries(wanted)) {
            if (on)
                this._block([name]);
            else
                this._unblockOne(name);
        }
    }

    _unblockOne(name) {
        const id = this._blocked?.get(name);
        if (!id)
            return;
        GObject.signal_handler_unblock(global.window_manager, id);
        this._blocked.delete(name);
    }

    _block(names = [...DROPLET, 'map']) {
        for (const name of names) {
            if (this._blocked.has(name))
                continue;
            // The first handler on the signal is GNOME's: the window manager connects it before any extension loads
            const id = GObject.signal_handler_find(global.window_manager, {signalId: name});
            if (!id) {
                console.warn(`Amethystora UI: motion: GNOME's ${name} handler was not found`);
                continue;
            }
            GObject.signal_handler_block(global.window_manager, id);
            this._blocked.set(name, id);
        }
    }

    _unblock() {
        for (const id of this._blocked?.values() ?? [])
            GObject.signal_handler_unblock(global.window_manager, id);
        this._blocked?.clear();
    }

    // ---- The window manager's events ----

    // A window closed: it leaves as the open-close key says, or as GNOME has it
    _destroy(shellwm, actor) {
        if (!this._blocked?.has('destroy'))
            return;
        const style = this._arrival;
        if (style && this._animated(actor) && this._arrivals.close(shellwm, actor, style, this._reduced))
            return;
        Main.wm._destroyWindow(shellwm, actor);
    }

    _minimize(shellwm, actor) {
        if (!this._blocked?.has('minimize'))
            return;
        const resume = this._takeResume(actor);
        if (!this._animated(actor)) {
            Main.wm._minimizeWindow(shellwm, actor);
            return;
        }
        if (!Main.wm._shouldAnimateActor(actor, TYPES)) {
            shellwm.completed_minimize(actor);
            return;
        }
        Main.wm._minimizing.add(actor);
        this._flySafely('minimize', actor, resume);
    }

    _unminimize(shellwm, actor) {
        if (!this._blocked?.has('unminimize'))
            return;
        const resume = this._takeResume(actor);
        if (!this._animated(actor)) {
            Main.wm._unminimizeWindow(shellwm, actor);
            return;
        }
        if (!Main.wm._shouldAnimateActor(actor, TYPES)) {
            shellwm.completed_unminimize(actor);
            return;
        }
        Main.wm._unminimizing.add(actor);
        this._flySafely('unminimize', actor, resume);
    }

    _map(shellwm, actor) {
        if (!this._blocked?.has('map'))
            return;
        const window = actor.meta_window;
        // A window a saved set opened rises into its place instead (asked once per window)
        if (this._droplet && this._ext.modules?.get('workspaces')?.arriving?.(window) && St.Settings.get().enable_animations &&
            Main.wm._getAnimationWindowType(actor) === Meta.WindowType.NORMAL) {
            this._rise(shellwm, actor);
            return;
        }
        const app = this._droplet && St.Settings.get().enable_animations &&
            Main.wm._getAnimationWindowType(actor) === Meta.WindowType.NORMAL ? this._takeLaunch(window) : null;
        if (!app) {
            // As the open-close key says, or as GNOME has it
            const style = this._arrival;
            if (style && this._animated(actor) && this._arrivals.open(shellwm, actor, style, this._reduced))
                return;
            Main.wm._mapWindow(shellwm, actor);
            return;
        }
        const ok = Main.wm._shouldAnimateActor(actor, [Meta.WindowType.NORMAL]);
        // GNOME's own map runs either way, for what it does besides animating (it follows the window's type and
        // dims a parent); told to skip the effect, it ends at once, in Mutter's eyes (shellwm) or in ours (stand)
        Main.wm.skipNextEffect(actor);
        if (!ok) {
            Main.wm._mapWindow(shellwm, actor);
            return;
        }
        const stand = {completed_map() {}};
        Main.wm._mapWindow(stand, actor);
        Main.wm._mapping.add(actor);
        this._flySafely('open', actor, null, app);
    }

    // A flight that cannot start (a dock half rebuilt, a window gone odd) must not leave Mutter's effect open: the
    // window ends at once, as GNOME's own ending leaves it
    _flySafely(event, actor, resume, app = null) {
        try {
            this._fly(event, actor, resume, app);
        } catch (e) {
            console.error(`Amethystora UI: motion: a window's ${event} was not drawn: ${e.message}`);
            const flight = this._flights?.get(actor);
            if (flight) {
                this._flights.delete(actor);
                try {
                    flight.stop();
                } catch {}
                this._dropProbe(flight);
                flight.detach();
            }
            const wm = global.window_manager;
            if (event === 'minimize') {
                Main.wm._minimizeWindowDone(wm, actor);
            } else {
                actor.show();
                if (event === 'open')
                    Main.wm._mapWindowDone(wm, actor);
                else
                    Main.wm._unminimizeWindowDone(wm, actor);
            }
        }
    }

    // Mutter ends every effect a window has before it starts another. GNOME's own handler, which runs first, has
    // ended a minimize or an open already; an unminimize is ours to end. The flight's last state is kept for the new
    // effect, which Mutter starts before the next frame, to turn round from.
    _kill(shellwm, actor) {
        const rise = this._risings?.get(actor);
        if (rise)
            this._endRise(rise);
        const flight = this._flights?.get(actor);
        if (!flight)
            return;
        this._flights.delete(actor);
        const rest = flight.restTranslation();
        const last = flight.stop();
        this._dropProbe(flight);
        if (flight.event === 'unminimize')
            Main.wm._unminimizeWindowDone(shellwm, actor);
        else if (flight.event === 'minimize')
            Main.wm._minimizeWindowDone(shellwm, actor);
        else
            Main.wm._mapWindowDone(shellwm, actor);
        if (!flight.gone)
            actor.set_translation(rest.x, rest.y, 0);
        this._glassChanged(flight);
        flight.detach();
        if (!last || flight.gone)
            return;
        this._resume.set(actor, {last, droplet: flight.droplet, geo: flight.geo});
        if (!this._resumeLater) {
            this._resumeLater = global.compositor.get_laters().add(Meta.LaterType.BEFORE_REDRAW, () => {
                this._resumeLater = 0;
                this._resume?.clear();
                return false;
            });
        }
    }

    _takeResume(actor) {
        const resume = this._resume?.get(actor) ?? null;
        this._resume?.delete(actor);
        return resume;
    }

    // Whether a window's going and coming is drawn here: not with GNOME's animations off, not for a window kept in
    // view (it goes out of sight its own way)
    _animated(actor) {
        if (!St.Settings.get().enable_animations)
            return false;
        return !this._ext.modules?.get('windows')?.isKept?.(actor.meta_window);
    }

    // The app the dock opened, if this window is its first since, and it was not long ago
    _takeLaunch(window) {
        const launch = this._launch;
        if (!launch)
            return null;
        if (GLib.get_monotonic_time() - launch.at > LAUNCH_WINDOW) {
            this._launch = null;
            return null;
        }
        if (Main.overview.visible)
            return null;
        const app = Shell.WindowTracker.get_default().get_window_app(window);
        if (app !== launch.app)
            return null;
        this._launch = null;
        return app;
    }

    // ---- Flights ----

    // event: minimize, unminimize or open
    _fly(event, actor, resume, app = null) {
        const window = actor.meta_window;
        const reduced = this._reduced;
        const out = event === 'minimize';
        let kind = reduced ? (out ? 'fade-out' : 'fade-in') : event;
        let u0 = 0;
        let from = null;
        let scale = 1;
        let geo = null;
        // Turned round part way: from where it was, if it was the same kind of motion
        if (resume && resume.droplet === !reduced) {
            const turn = turnRound(resume.last);
            if (reduced || turn.kind === (out ? 'minimize' : 'unminimize')) {
                if (!reduced)
                    kind = out ? 'minimize' : 'unminimize';
                u0 = turn.u0;
                from = turn.from;
                scale = turn.scale;
                geo = resume.geo;
            }
        }

        const f = window.get_frame_rect();
        const sf = scaleFactor();
        const square = window.maximized_horizontally || window.maximized_vertically || window.is_fullscreen();
        const R0 = square ? 0 : (this._ext.modules?.get('style')?.look?.window ?? RADIUS) * sf;
        const icon = geo ? {Ic: geo.Ic, S: geo.S, ring: geo.ring} : this._icon(window, event === 'open' ? app : null);
        const flightGeo = {W: f.width, H: f.height, R0, S: icon.S, Ic: icon.Ic, ring: icon.ring};

        // Where the window rests: as it was, on the way out; on the way back, where the tiling's strip draws it now
        const t0 = {x: actor.translation_x, y: actor.translation_y};
        const rest = out ? () => t0 : m => this._tilingRest(actor, m, t0);

        const probe = !reduced ? this._probe() : null;
        const accent = probe ? this._accent(probe) : ACCENT;
        const flight = new Flight(kind, actor, {
            geo: flightGeo, rest, from, u0, scale,
            shader: this._shader, accent,
            rippleAt: OUT.ripple,
            // The ring is the flight's no longer once it ripples: it goes by itself
            ripple: kind === 'minimize' && icon.ring && probe ? copy => {
                flight.probe = null;
                this._ripple(probe, icon, copy);
            } : null,
            chrome: this._chrome(),
            onEnd: f2 => this._end(f2),
            onGone: f2 => this._forget(f2),
        });
        flight.event = event;
        flight.geo = flightGeo;
        flight.probe = probe;
        this._flights.set(actor, flight);
        this._glassChanged(flight);
        flight.start();
    }

    // GNOME's map runs as for the dock's windows, told to skip its effect, and the rise takes the window from there;
    // it ends with GNOME's own ending, as the flights do
    _rise(shellwm, actor) {
        const ok = Main.wm._shouldAnimateActor(actor, [Meta.WindowType.NORMAL]);
        Main.wm.skipNextEffect(actor);
        if (!ok) {
            Main.wm._mapWindow(shellwm, actor);
            return;
        }
        Main.wm._mapWindow({completed_map() {}}, actor);
        Main.wm._mapping.add(actor);
        const t0 = {x: actor.translation_x, y: actor.translation_y};
        const rise = new Rise(actor, {
            reduced: this._reduced,
            rest: () => this._tilingRest(actor, actor.meta_window?.get_frame_rect(), t0),
            onEnd: r => this._endRise(r),
        });
        this._risings.set(actor, rise);
        this.risen++;
        try {
            rise.start();
        } catch (e) {
            console.error(`Amethystora UI: motion: a window of a set did not rise: ${e.message}`);
            this._endRise(rise);
            return;
        }
        this._ext.modules?.get('glass')?.windows?._queue?.(actor.meta_window);
    }

    _endRise(rise) {
        const actor = rise.actor;
        if (this._risings?.get(actor) !== rise)
            return;
        this._risings.delete(actor);
        const rest = rise.restTranslation();
        rise.stop();
        actor.show();
        Main.wm._mapWindowDone(global.window_manager, actor);
        actor.set_translation(rest.x, rest.y, 0);
        const window = actor.meta_window;
        if (window)
            this._ext.modules?.get('glass')?.windows?._queue?.(window);
    }

    // Where a window rests in the tiling's strip, as a translation, or `fallback` when it is in none
    // m: the frame as measured this frame (flight.js measure())
    _tilingRest(actor, m, fallback) {
        const info = this._ext.modules?.get('tiling')?.engine?.info?.(actor.meta_window);
        if (!info?.strip || !info.vis)
            return fallback;
        return {x: info.vis.x - m.x, y: info.vis.y - m.y};
    }

    // The flight is over: what it put on the window comes off, and the window manager's own ending puts the window
    // as GNOME leaves it
    _end(flight) {
        const actor = flight.actor;
        if (this._flights?.get(actor) !== flight)
            return;
        this._flights.delete(actor);
        const rest = flight.restTranslation();
        flight.stop();
        this._dropProbe(flight);
        const wm = global.window_manager;
        // Ending an effect may let Mutter destroy a window closed meanwhile: nothing touches the actor after that
        if (flight.event === 'open') {
            Main.wm._mapWindowDone(wm, actor);
            // GNOME's ending puts the window where its frame is; the tiling's strip may draw it elsewhere
            if (!flight.gone)
                actor.set_translation(rest.x, rest.y, 0);
        } else {
            actor.set_translation(rest.x, rest.y, 0);
            if (flight.event === 'minimize')
                Main.wm._minimizeWindowDone(wm, actor);
            else
                Main.wm._unminimizeWindowDone(wm, actor);
        }
        this._glassChanged(flight);
        flight.detach();
    }

    // A window whose actor is destroyed in the middle of a flight (Mutter ends its effects first, so it should not be)
    _forget(flight) {
        if (this._flights?.get(flight.actor) !== flight)
            return;
        this._flights.delete(flight.actor);
        flight.stop();
        this._dropProbe(flight);
    }

    // The ring a flight made goes with it, unless it is rippling
    _dropProbe(flight) {
        if (flight.probe && !this._ripples?.has(flight.probe))
            flight.probe.destroy();
        flight.probe = null;
    }

    _glassChanged(flight) {
        if (!flight.gone)
            this._ext.modules?.get('glass')?.windows?._queue?.(flight.window);
    }

    // Where the top bar and the dock are on the stage now: a drop on its way back is drawn over them only until it
    // is clear of them
    _chrome() {
        const boxes = [];
        const add = actor => {
            if (!actor?.mapped)
                return;
            const e = actor.get_transformed_extents();
            boxes.push({x1: e.origin.x, y1: e.origin.y, x2: e.origin.x + e.size.width, y2: e.origin.y + e.size.height});
        };
        add(Main.layoutManager.panelBox);
        for (const surface of this._ext.modules?.get('dock')?.glassSurfaces?.() ?? [])
            add(surface.actor);
        return boxes;
    }

    // ---- The icon, the ring and the accent ----

    // Where the window's icon is: the dock's own picture of it, else what GNOME was told of the dock's button, else
    // the bottom centre of the window's screen, where no ring ripples
    _icon(window, app) {
        const dock = this._ext.modules?.get('dock');
        const box = app ? dock?.iconBoxForApp?.(app, window.get_monitor()) : dock?.iconBoxFor?.(window);
        if (box && box.width > 0 && box.height > 0) {
            return {
                Ic: {x: box.x + box.width / 2, y: box.y + box.height / 2},
                S: Math.min(box.width, box.height), ring: true,
            };
        }
        if (!app) {
            const [ok, rect] = window.get_icon_geometry();
            if (ok && rect.width > 0 && rect.height > 0) {
                return {
                    Ic: {x: rect.x + rect.width / 2, y: rect.y + rect.height / 2},
                    S: 0.75 * Math.min(rect.width, rect.height), ring: true,
                };
            }
        }
        const sf = scaleFactor();
        const monitor = Main.layoutManager.monitors[window.get_monitor()] ?? Main.layoutManager.primaryMonitor;
        if (!monitor) {
            const c = frameCentre(window);
            return {Ic: c, S: NO_ICON_SIZE * sf, ring: false};
        }
        return {
            Ic: {x: monitor.x + monitor.width / 2, y: monitor.y + monitor.height - NO_ICON_LIFT * sf},
            S: NO_ICON_SIZE * sf, ring: false,
        };
    }

    // The ring, made with each droplet flight, unseen until it ripples: its style carries the theme's accent
    _probe() {
        const probe = new St.Widget({style_class: 'amethystora-motion-ripple', reactive: false, opacity: 0});
        probe.hide();
        Main.layoutManager.uiGroup.add_child(probe);
        return probe;
    }

    _accent(probe) {
        try {
            const [ok, color] = probe.get_theme_node().lookup_color('-amethystora-accent', false);
            if (ok)
                return [color.red / 255, color.green / 255, color.blue / 255];
        } catch {}
        return ACCENT;
    }

    // The ring round the icon as the drop lands: it comes up, then widens and fades
    _ripple(ring, icon, drop) {
        const size = icon.S;
        const sf = scaleFactor();
        this._ripples.add(ring);
        ring.set_style(`border-radius: ${size / 2 / sf}px;`);
        ring.set_size(size, size);
        ring.set_position(Math.round(icon.Ic.x - size / 2), Math.round(icon.Ic.y - size / 2));
        ring.set_pivot_point(0.5, 0.5);
        ring.set_scale(0.6, 0.6);
        ring.opacity = 0;
        // Over everything but the drop landing in it
        const group = Main.layoutManager.uiGroup;
        if (drop?.get_parent() === group)
            group.set_child_below_sibling(ring, drop);
        else
            group.set_child_below_sibling(ring, global.top_window_group);
        ring.show();
        // Once, however it ends (the module stopping destroys it itself)
        const done = () => {
            if (this._ripples?.delete(ring))
                ring.destroy();
        };
        ring.ease({
            opacity: 255, scale_x: 0.8, scale_y: 0.8,
            duration: RIPPLE_IN_MS, mode: Clutter.AnimationMode.EASE,
            onStopped: finished => {
                if (!finished) {
                    done();
                    return;
                }
                ring.ease({
                    opacity: 0, scale_x: 1.9, scale_y: 1.9,
                    duration: RIPPLE_OUT_MS, mode: Clutter.AnimationMode.EASE,
                    onStopped: done,
                });
            },
        });
    }

    // ---- Changing workspace ----

    _hook() {
        if (this._hookId)
            return;
        this._hookId = Main.uiGroup.connect('child-added', (_, child) => this._childAdded(child));
    }

    // A switch under way finishes as it began: its driver ends with its group
    _unhook() {
        if (this._hookId)
            Main.uiGroup.disconnect(this._hookId);
        this._hookId = 0;
    }

    _childAdded(child) {
        if (!(child instanceof WorkspaceAnimation.MonitorGroup) || !St.Settings.get().enable_animations)
            return;
        const driver = new Depth(child, this._reduced ? 'fade' : 'depth');
        this._drivers.add(driver);
        child.connectObject('destroy', () => this._drivers?.delete(driver), this);
    }
}
