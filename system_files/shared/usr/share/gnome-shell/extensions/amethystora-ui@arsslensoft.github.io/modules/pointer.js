// The pointer, easier to find. Shake the mouse from side to side and the pointer swells to three and a half times its
// size (or grows a little and sends rings out, or nothing, as `ame desktop pointer-shake` says), then settles a moment
// after the shaking stops. On a screen with small pixels shown at 100% the pointer is drawn one size larger, unless the
// person chose a size of their own. The pointer's look is the theme's (amethystora-theme), not this module's.
//
// Nothing runs while the mouse is still: the shake is read from the pointer's own moves, and only while shaking is on.
// While the pointer is shown large it is a copy of the real one, which is hidden for that time and always given back,
// whatever ends the flare.

import Clutter from 'gi://Clutter';
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import Graphene from 'gi://Graphene';
import Mtk from 'gi://Mtk';
import St from 'gi://St';

import * as Config from 'resource:///org/gnome/shell/misc/config.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import {gettext as _} from 'resource:///org/gnome/shell/extensions/extension.js';

import {ShakeDetector, STEP} from './pointer/shake.js';
import {denseSize} from './pointer/dense.js';
import {State} from './pointer/state.js';

const SCHEMA = 'org.gnome.shell.extensions.amethystora-ui.pointer';

const SCALES = {grow: 3.4, beacon: 1.7};
const GROW_MS = 220; // the swell and the settle, before the shell's slow-down factor
const HOLD_MS = 1100; // how long the pointer stays large after the last shaking move
const SAFETY_MS = 2000; // the longest a settle may take before the pointer is given back regardless
const RING_SIZE = 80; // px, the beacon's rings before they spread
const RING_MS = 1000;
const RING_DELAYS = [0, 330, 660];
const RING_FROM = 0.3;
const RING_TO = 2.6;
const RING_OPACITY = 230;
const DBUS_TIMEOUT = 5000;
// The tip about shaking comes the first time the pointer moves after this long without input
const TIP_IDLE_MS = 10 * 60 * 1000;
// GNOME Shell 51 draws its pointers from the theme's SVG (cursors_scalable), 50 from its Xcursor files
const SVG_CURSORS = parseInt(Config.PACKAGE_VERSION) >= 51;

const BUTTONS = Clutter.ModifierType.BUTTON1_MASK | Clutter.ModifierType.BUTTON2_MASK |
    Clutter.ModifierType.BUTTON3_MASK | Clutter.ModifierType.BUTTON4_MASK | Clutter.ModifierType.BUTTON5_MASK;

// The swell's curve: it overshoots a little before it settles, on the way up and on the way down
const CURVE = [new Graphene.Point({x: 0.2, y: 0.9}), new Graphene.Point({x: 0.25, y: 1.25})];

export class Pointer {
    constructor(ext) {
        this._ext = ext;
    }

    enable() {
        this.stats = {
            connected: false, samples: 0, fired: 0, hidden: false, actors: 0, rings: 0, ringsMax: 0, maxScale: 0,
            handlerUsMax: 0, handlerUsSum: 0, frameUsMax: 0,
        };
        this.denseSize = denseSize;
        this._tracker = global.backend.get_cursor_tracker();
        this._seat = global.stage.context.get_backend().get_default_seat();
        // The stage's pointer, kept for the life of the stage
        this._sprite0 = global.stage.context.get_backend().get_pointer_sprite(global.stage);
        this._last = 0;
        this._detector = null;
        this._flare = null;
        this._hidden = false;
        this._unfocus = false;
        this._denseOn = false;

        this._settings = this._ext.getSettings(SCHEMA);
        this._settings.connectObject(
            'changed::shake', () => this._syncShake(),
            'changed::dense', () => this._syncDense(),
            this);
        this._syncShake();
        this._syncDense();
    }

    disable() {
        // The pointer back first, whatever else fails
        this._endFlare();
        this.disarmTip();
        this._tracker?.disconnectObject(this);
        this._detector = null;
        this._stopDense();
        this._settings?.disconnectObject(this);
        // Disposed now rather than by the collector (dock.js says why)
        this._settings?.run_dispose();
        this._settings = null;
        this._tracker = null;
        this._seat = null;
        this._sprite0 = null;
    }

    // ------------------------------------------------------------------ shake --

    _syncShake() {
        this._endFlare();
        const on = this._settings.get_string('shake') !== 'off';
        if (on && !this.stats.connected) {
            this._detector = new ShakeDetector();
            this._tracker.connectObject('position-invalidated', () => this._onMove(), this);
            this.stats.connected = true;
            this.armTip();
        } else if (!on && this.stats.connected) {
            this._tracker.disconnectObject(this);
            this._detector = null;
            this.stats.connected = false;
            this.disarmTip();
        }
    }

    // The tip (the hints module): Mutter's own idle monitor says when ten minutes passed without input, and then when
    // input comes back; nothing is counted here. Armed only while tips are on (the hints module arms and disarms it
    // as they are turned on and off) and the shake is, and never once the tip has been shown.
    armTip() {
        const hints = this._ext.modules?.get('hints');
        if (this._tipWatch || !this.stats?.connected || !hints?.enabled || hints.wasShown('shake'))
            return;
        const monitor = global.backend.get_core_idle_monitor();
        this._tipWatch = monitor.add_idle_watch(TIP_IDLE_MS, (_monitor, id) => {
            // An idle watch fires each time; this one is wanted once
            monitor.remove_watch(id);
            this._tipWatch = monitor.add_user_active_watch(() => {
                this._tipWatch = 0;
                const text = _('Tip: shake the mouse to find the pointer.');
                if (!hints?.offer('shake', {text, anchor: 'pointer'}))
                    this.armTip();
            });
        });
    }

    disarmTip() {
        if (this._tipWatch)
            global.backend.get_core_idle_monitor().remove_watch(this._tipWatch);
        this._tipWatch = 0;
    }

    // Every pointer motion: the large copy follows each one while it is shown; the shake is looked at once every STEP
    // ms at most, and never counts while a button is held
    _onMove() {
        try {
            if (this._flare)
                this._follow(this._position());
            const now = GLib.get_monotonic_time();
            if (now - this._last < STEP * 1000)
                return;
            this._last = now;
            this.stats.samples++;
            const shaken = this._look(now);
            // The cost of looking, which every move pays; starting a flare is paid once a shake
            const spent = GLib.get_monotonic_time() - now;
            this.stats.handlerUsMax = Math.max(this.stats.handlerUsMax, spent);
            this.stats.handlerUsSum += spent;
            if (!shaken)
                return;
            // The buttons are asked of the seat only now: a drag or a stroke of a drawing is never a shake
            const [, mods] = this._tracker.get_pointer();
            if (mods & BUTTONS)
                this._detector.reset();
            else
                this._onShake();
        } catch (e) {
            console.error(`Amethystora UI: the pointer's shake: ${e.message}`);
            this._endFlare();
        }
    }

    // Where the pointer is, as the stage last saw it move. The seat's own answer (get_pointer) takes a lock the input
    // thread also takes for every event it reads; the stage's pointer sprite holds the same position on this thread.
    _position() {
        return this._sprite0?.get_coords() ?? this._tracker.get_pointer()[0];
    }

    // One look at the pointer for the shake, at now (µs): true while the movement so far is a shake. Apart from the
    // handler so that the lab can time it alone (suite 12, V14).
    _look(now) {
        return this._detector.sample(this._position().x, now / 1000);
    }

    _onShake() {
        // Never over someone else's hiding of the pointer: the magnifier, touch, the lock screen, a game
        if (!this._tracker.get_pointer_visible() && !this._hidden)
            return;
        if (Main.sessionMode.isLocked || Main.magnifier?.isActive?.())
            return;
        // A fullscreen game or video may move its own pointer from side to side: it is never hidden for a flare
        if (global.display.focus_window?.is_fullscreen())
            return;
        try {
            if (this._flare) {
                this._rearm();
                return;
            }
            if (this._startFlare())
                this.stats.fired++;
        } catch (e) {
            console.error(`Amethystora UI: the pointer's flare: ${e.message}`);
            this._endFlare();
        }
    }

    // The sprite and its hot spot in logical pixels, or null when there is nothing to show (a client hid its pointer).
    // Mutter answers only while the seat keeps its focus, which is asked for first.
    _sprite() {
        if (!this._unfocus) {
            this._seat.inhibit_unfocus();
            this._unfocus = true;
        }
        const texture = this._tracker.get_sprite();
        if (!texture || texture.get_width() <= 1 || texture.get_height() <= 1)
            return null;
        const [hx, hy] = this._tracker.get_hot();
        const scale = this._spriteScale();
        return {texture, hx, hy, scale, width: texture.get_width() / scale, height: texture.get_height() / scale};
    }

    // How many texture pixels make one pixel of the stage under the pointer. Where the stage is laid out in logical
    // pixels (its view under the pointer has a scale above 1), GNOME 50 loads an Xcursor image at that scale rounded
    // up and GNOME 51 draws the SVG at the scale itself; where it is laid out in the screens' own pixels (the view's
    // scale is 1), the texture is drawn as it is.
    _spriteScale() {
        const point = this._position();
        const rect = new Mtk.Rectangle();
        const view = global.stage.peek_stage_views().find(v => {
            v.get_layout(rect);
            return point.x >= rect.x && point.x < rect.x + rect.width && point.y >= rect.y && point.y < rect.y + rect.height;
        });
        const viewScale = view?.get_scale() ?? 1;
        return SVG_CURSORS ? viewScale : Math.ceil(viewScale);
    }

    _startFlare() {
        const sprite = this._sprite();
        if (!sprite) {
            this._endFlare();
            return false;
        }
        const mode = this._settings.get_string('shake');
        const flare = {mode, to: SCALES[mode] ?? SCALES.grow, sprite, copy: null, rings: [], timeline: null, hold: 0,
            safety: 0};
        this._flare = flare;
        this.stats.maxScale = 0;
        this.stats.ringsMax = 0;

        const copy = new Clutter.Actor({reactive: false});
        flare.copy = copy;
        this._setSprite(sprite);
        copy.set_content_scaling_filters(Clutter.ScalingFilter.TRILINEAR, Clutter.ScalingFilter.LINEAR);
        global.stage.add_child(copy);
        global.stage.set_child_above_sibling(copy, null);
        this.stats.actors = 1;

        if (mode === 'beacon') {
            const size = RING_SIZE * St.ThemeContext.get_for_stage(global.stage).scale_factor;
            for (let i = 0; i < RING_DELAYS.length; i++) {
                const ring = new St.Widget({style_class: 'amethystora-pointer-ring', reactive: false,
                    width: size, height: size, pivot_point: new Graphene.Point({x: 0.5, y: 0.5})});
                global.stage.insert_child_below(ring, copy);
                flare.rings.push(ring);
            }
            this.stats.rings = this.stats.ringsMax = flare.rings.length;
        }
        this._follow(this._position());

        // Everything is made: now the real pointer can go
        this._tracker.inhibit_cursor_visibility();
        this._hidden = true;
        this.stats.hidden = true;

        this._tracker.connectObject('cursor-changed', () => this._onCursorChanged(), flare);
        this._animate(1, flare.to);
        this._startRings();
        this._rearm();
        return true;
    }

    _setSprite(sprite) {
        const {copy} = this._flare;
        copy.set_content(Clutter.TextureContent.new_from_texture(sprite.texture, null));
        copy.set_size(sprite.width, sprite.height);
        copy.set_pivot_point(sprite.hx / sprite.texture.get_width(), sprite.hy / sprite.texture.get_height());
        this._flare.sprite = sprite;
    }

    // The shape under the pointer changed, or an animated pointer moved on a frame
    _onCursorChanged() {
        try {
            const sprite = this._sprite();
            if (!sprite) {
                this._endFlare();
                return;
            }
            this._setSprite(sprite);
            const point = this._position();
            this._follow(point);
        } catch (e) {
            console.error(`Amethystora UI: the pointer's shake: ${e.message}`);
            this._endFlare();
        }
    }

    _follow(point) {
        const {copy, rings, sprite} = this._flare;
        copy.set_position(Math.round(point.x - sprite.hx / sprite.scale), Math.round(point.y - sprite.hy / sprite.scale));
        for (const ring of rings)
            ring.set_position(Math.round(point.x - ring.width / 2), Math.round(point.y - ring.height / 2));
    }

    // Hold the pointer large for HOLD_MS more; a shake that comes back while it settles grows it again
    _rearm() {
        const flare = this._flare;
        if (flare.hold)
            GLib.source_remove(flare.hold);
        if (flare.settling) {
            flare.settling = false;
            if (flare.safety)
                GLib.source_remove(flare.safety);
            flare.safety = 0;
            this._animate(flare.copy.scale_x, flare.to);
        }
        flare.hold = GLib.timeout_add(GLib.PRIORITY_DEFAULT, HOLD_MS, () => {
            flare.hold = 0;
            this._guard(() => this._settle());
            return GLib.SOURCE_REMOVE;
        });
    }

    // A callback's body, which ends the flare (and so shows the pointer) if it throws
    _guard(body) {
        try {
            body();
        } catch (e) {
            console.error(`Amethystora UI: the pointer's flare: ${e.message}`);
            this._endFlare();
        }
    }

    _settle() {
        const flare = this._flare;
        if (!flare)
            return;
        if (!St.Settings.get().enable_animations) {
            this._endFlare();
            return;
        }
        flare.settling = true;
        // Should the settle never complete, the pointer comes back all the same
        if (!flare.safety) {
            flare.safety = GLib.timeout_add(GLib.PRIORITY_DEFAULT, SAFETY_MS, () => {
                flare.safety = 0;
                if (this._flare === flare)
                    this._endFlare();
                return GLib.SOURCE_REMOVE;
            });
        }
        this._animate(flare.copy.scale_x, 1, () => this._endFlare());
    }

    // The copy's scale from one value to another on the swell's curve, which overshoots both ways
    _animate(from, to, done = null) {
        const flare = this._flare;
        flare.timeline?.stop();
        flare.timeline?.disconnectObject(this);
        flare.timeline = null;
        const settings = St.Settings.get();
        if (!settings.enable_animations) {
            this._scale(to);
            return;
        }
        const timeline = new Clutter.Timeline({actor: flare.copy, duration: Math.max(1, Math.round(GROW_MS * settings.slow_down_factor))});
        timeline.set_cubic_bezier_progress(...CURVE);
        timeline.connectObject(
            'new-frame', () => this._guard(() => {
                const start = GLib.get_monotonic_time();
                this._scale(from + (to - from) * timeline.get_progress());
                this.stats.frameUsMax = Math.max(this.stats.frameUsMax, GLib.get_monotonic_time() - start);
            }),
            'completed', () => this._guard(() => {
                this._scale(to);
                if (done)
                    done();
            }), this);
        flare.timeline = timeline;
        timeline.start();
    }

    _scale(value) {
        const {copy} = this._flare;
        copy.set_scale(value, value);
        this.stats.maxScale = Math.max(this.stats.maxScale, value);
    }

    _startRings() {
        const {rings} = this._flare;
        if (!rings.length)
            return;
        if (!St.Settings.get().enable_animations) {
            // One still ring for the hold
            rings.slice(1).forEach(ring => ring.destroy());
            rings.length = 1;
            rings[0].set({scale_x: 1, scale_y: 1, opacity: RING_OPACITY});
            this.stats.rings = 1;
            return;
        }
        rings.forEach((ring, i) => {
            ring.set({scale_x: RING_FROM, scale_y: RING_FROM, opacity: RING_OPACITY});
            ring.ease({
                scale_x: RING_TO, scale_y: RING_TO, opacity: 0, duration: RING_MS, delay: RING_DELAYS[i],
                mode: Clutter.AnimationMode.EASE_OUT, repeatCount: -1,
            });
        });
    }

    // The only way a flare ends, and safe to call at any time: each step on its own, so that the pointer always comes
    // back and the counters Mutter keeps stay even
    _endFlare() {
        const flare = this._flare;
        this._flare = null;
        if (flare) {
            const steps = [
                () => flare.hold && GLib.source_remove(flare.hold),
                () => flare.safety && GLib.source_remove(flare.safety),
                () => {
                    flare.timeline?.stop();
                    flare.timeline?.disconnectObject(this);
                },
                () => this._tracker?.disconnectObject(flare),
                () => flare.rings.forEach(ring => {
                    ring.remove_all_transitions();
                    ring.destroy();
                }),
                () => flare.copy?.destroy(),
            ];
            for (const step of steps) {
                try {
                    step();
                } catch (e) {
                    console.error(`Amethystora UI: ending the pointer's flare: ${e.message}`);
                }
            }
            if (this.stats) {
                this.stats.actors = 0;
                this.stats.rings = 0;
            }
        }
        if (this._hidden) {
            this._hidden = false;
            try {
                this._tracker.uninhibit_cursor_visibility();
            } catch (e) {
                console.error(`Amethystora UI: showing the pointer again: ${e.message}`);
            }
            if (this.stats)
                this.stats.hidden = false;
        }
        if (this._unfocus) {
            this._unfocus = false;
            try {
                this._seat.uninhibit_unfocus();
            } catch (e) {
                console.error(`Amethystora UI: giving the pointer's focus back: ${e.message}`);
            }
        }
    }

    // ------------------------------------------------------------------ dense --

    _syncDense() {
        const on = this._settings.get_boolean('dense');
        if (on === this._denseOn)
            return;
        if (on) {
            this._denseOn = true;
            this._interface = new Gio.Settings({schema_id: 'org.gnome.desktop.interface'});
            this._state = new State();
            this._interface.connectObject('changed::cursor-size', () => this._onSizeChanged(), this);
            global.backend.get_monitor_manager().connectObject('monitors-changed', () => this._readScreen(), this);
            this._readScreen();
        } else {
            // Turned off: the size it wrote goes back to the image's
            if (this._userSize() !== null && this._userSize() === this._state.wrote)
                this._resetSize();
            this._state.wrote = null;
            this._stopDense();
        }
    }

    // Stops listening; the size stays as it is, since the extension also stops at every screen lock
    _stopDense() {
        this._denseOn = false;
        this._cancellable?.cancel();
        this._cancellable = null;
        this._interface?.disconnectObject(this);
        this._interface?.run_dispose();
        this._interface = null;
        global.backend.get_monitor_manager().disconnectObject(this);
        this._state = null;
    }

    _userSize() {
        return this._interface.get_user_value('cursor-size')?.unpack() ?? null;
    }

    _onSizeChanged() {
        if (this._writing || !this._state)
            return;
        // The person set a size: it is theirs from now on
        if (this._userSize() !== this._state.wrote)
            this._state.wrote = null;
    }

    _setSize(size) {
        this._writing = true;
        try {
            this._interface.set_int('cursor-size', size);
        } finally {
            this._writing = false;
        }
    }

    _resetSize() {
        this._writing = true;
        try {
            this._interface.reset('cursor-size');
        } finally {
            this._writing = false;
        }
    }

    // The primary screen's size from Mutter's own display configuration, asked without waiting
    _readScreen() {
        this._cancellable?.cancel();
        const cancellable = new Gio.Cancellable();
        this._cancellable = cancellable;
        Gio.DBus.session.call('org.gnome.Mutter.DisplayConfig', '/org/gnome/Mutter/DisplayConfig',
            'org.gnome.Mutter.DisplayConfig', 'GetResources', null, null, Gio.DBusCallFlags.NONE, DBUS_TIMEOUT,
            cancellable, (connection, result) => {
                try {
                    const reply = connection.call_finish(result);
                    if (cancellable.is_cancelled() || !this._denseOn)
                        return;
                    const info = this._screenInfo(reply);
                    if (info)
                        this.applyDense(info);
                } catch (e) {
                    if (!e.matches?.(Gio.IOErrorEnum, Gio.IOErrorEnum.CANCELLED))
                        console.warn(`Amethystora UI: the pointer could not read the screen's size: ${e.message}`);
                } finally {
                    if (this._cancellable === cancellable)
                        this._cancellable = null;
                }
            });
    }

    _screenInfo(reply) {
        const manager = global.backend.get_monitor_manager();
        const primary = manager.get_monitors().find(m => m.is_primary());
        if (!primary)
            return null;
        const connector = primary.get_connector();
        const outputs = reply.get_child_value(2);
        for (let i = 0; i < outputs.n_children(); i++) {
            const output = outputs.get_child_value(i);
            if (output.get_child_value(4).get_string()[0] !== connector)
                continue;
            const props = output.get_child_value(7).recursiveUnpack();
            const index = manager.get_monitor_for_connector(connector);
            if (index < 0)
                return null;
            const scale = global.display.get_monitor_scale(index);
            const geometry = global.display.get_monitor_geometry(index);
            return {
                widthPx: geometry.width * scale, heightPx: geometry.height * scale,
                widthMm: props['width-mm'] ?? 0, heightMm: props['height-mm'] ?? 0, scale, tiled: 'tile' in props,
            };
        }
        return null;
    }

    // Write the larger size for a dense screen, or take back the one written for a screen that no longer is; never
    // over a size the person chose
    applyDense(info) {
        if (!this._state)
            return;
        const want = denseSize(info);
        const user = this._userSize();
        const wrote = this._state.wrote;
        if (user !== null && user !== wrote) {
            this._state.wrote = null;
        } else if (want !== null && user !== want) {
            this._setSize(want);
            this._state.wrote = want;
        } else if (want === null && user !== null && user === wrote) {
            this._resetSize();
            this._state.wrote = null;
        }
    }
}
