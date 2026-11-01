// A resting screen (R6-06, offered, off): left alone for a few minutes, the desktop rests. A large clock and the day
// over the wallpaper, which drifts slowly, what plays if something plays, and when the screen will lock, which is when
// it would have locked anyway: nothing here holds the lock back or brings it forward. The windows are hidden under it,
// a small privacy gain. Any key, a click or the pointer moving brings everything back, and the key that did it never
// reaches a window.
//
// It costs nothing until it rests: one idle watch of Mutter's (the same monitor GNOME's screen shield uses), and only
// while the key is on. It does not rest while something holds the session awake (gnome-session's idle inhibitor: a
// video playing, a presentation), while a window is fullscreen on the main screen, while the screen is shared or
// recorded, or while presenting. While resting, the clock is set at each turn of the minute, and the wallpaper drifts at
// a low frame rate (DRIFT_FPS), never on battery, in the power saver or with reduced motion, when it holds still.
//
// Keys (org.gnome.shell.extensions.amethystora-ui.rest, ame desktop resting-screen, Control):
//   enabled  false   rest at all
//   after    120     seconds left alone before it rests (60 to 1800)

import Clutter from 'gi://Clutter';
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import Pango from 'gi://Pango';
import Shell from 'gi://Shell';
import St from 'gi://St';

import {gettext as _} from 'resource:///org/gnome/shell/extensions/extension.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';

const SCHEMA = 'org.gnome.shell.extensions.amethystora-ui.rest';
const SESSION = 'org.gnome.desktop.session';
const SCREENSAVER = 'org.gnome.desktop.screensaver';
// gnome-session's flag for "the session may not go idle"
const INHIBIT_IDLE = 8;
// The drift: frames a second, how far the picture is enlarged to have room to move, and how long one slow sway takes
const DRIFT_FPS = 8;
const DRIFT_SCALE = 1.08;
const DRIFT_PERIOD_S = 90;
const FADE_MS = 400;
// GNOME's own fade before it locks (screenShield.js STANDARD_FADE_TIME), the shortest lock delay there is
const SHIELD_FADE_MS = 10000;

const UPOWER = 'org.freedesktop.UPower';
const UPOWER_PATH = '/org/freedesktop/UPower';

export class Rest {
    constructor(ext) {
        this._ext = ext;
    }

    enable() {
        this._settings = this._ext.getSettings(SCHEMA);
        this._settings.connectObject(
            'changed::enabled', () => this._sync(),
            'changed::after', () => this._sync(),
            this);
        this._sync();
    }

    disable() {
        this._end(false);
        this._unwatch();
        this._settings?.disconnectObject(this);
        // Disposed now rather than by the collector (dock.js says why)
        this._settings?.run_dispose();
        this._settings = null;
    }

    get _monitor() {
        return global.backend.get_core_idle_monitor();
    }

    // ---- Watching for the person to leave ----

    _sync() {
        this._unwatch();
        if (!this._settings.get_boolean('enabled'))
            return;
        const after = Math.min(1800, Math.max(60, this._settings.get_int('after')));
        this._idleWatch = this._monitor.add_idle_watch(after * 1000, () => this._idle());
    }

    _unwatch() {
        if (this._idleWatch)
            this._monitor.remove_watch(this._idleWatch);
        this._idleWatch = 0;
        this._askCancellable?.cancel();
        this._askCancellable = null;
    }

    // Left alone long enough: it rests unless something says it should not
    _idle() {
        if (this._layer || this._why() !== null)
            return;
        this._askCancellable?.cancel();
        const cancellable = new Gio.Cancellable();
        this._askCancellable = cancellable;
        Gio.DBus.session.call('org.gnome.SessionManager', '/org/gnome/SessionManager', 'org.gnome.SessionManager',
            'IsInhibited', new GLib.Variant('(u)', [INHIBIT_IDLE]), new GLib.VariantType('(b)'),
            Gio.DBusCallFlags.NO_AUTO_START, 1000, cancellable, (connection, result) => {
                let inhibited = false;
                try {
                    [inhibited] = connection.call_finish(result).deep_unpack();
                } catch (e) {
                    if (e.matches(Gio.IOErrorEnum, Gio.IOErrorEnum.CANCELLED))
                        return;
                }
                if (this._askCancellable !== cancellable)
                    return;
                this._askCancellable = null;
                // Asked again, now: the person may have come back while the answer came
                if (!inhibited && this._monitor.get_idletime() >= 1000 && this._why() === null)
                    this._rest();
            });
    }

    // Why it does not rest now, or null
    _why() {
        if (!this._settings?.get_boolean('enabled'))
            return 'off';
        if (Main.sessionMode.isLocked || Main.screenShield?.active)
            return 'locked';
        if (Main.layoutManager.primaryMonitor?.inFullscreen)
            return 'fullscreen';
        const capture = this._ext.modules?.get('pill')?.capture;
        if (capture?.sharing || capture?.recording)
            return 'sharing';
        if (this._ext.modules?.get('present')?.presenting)
            return 'presenting';
        return null;
    }

    // ---- Resting ----

    _rest() {
        const layer = new St.Widget({
            name: 'amethystora-rest',
            style_class: 'amethystora-rest',
            reactive: true,
            can_focus: true,
            opacity: 0,
        });
        layer.add_constraint(new Clutter.BindConstraint({source: global.stage, coordinate: Clutter.BindCoordinate.ALL}));
        // Under GNOME's own fade to the lock, which goes above everything
        Main.layoutManager.uiGroup.add_child(layer);
        this._layer = layer;
        this._pictures = [];
        for (const monitor of Main.layoutManager.monitors)
            this._pictures.push(this._picture(layer, monitor));
        this._face(layer);

        // The keys come here while it rests, never to the window under it
        try {
            this._grab = Main.pushModal(layer, {actionMode: Shell.ActionMode.POPUP});
        } catch (e) {
            console.error(`Amethystora UI: the resting screen did not take the keyboard: ${e.message}`);
        }
        layer.grab_key_focus();
        this._activeWatch = this._monitor.add_user_active_watch(() => {
            this._activeWatch = 0;
            this._end(true);
        });
        Main.screenShield?.connectObject('active-changed', () => this._end(false), this);
        Main.layoutManager.connectObject('monitors-changed', () => this._end(false), this);
        this._tick();
        this._syncDrift();
        layer.ease({opacity: 255, duration: FADE_MS, mode: Clutter.AnimationMode.EASE_OUT_QUAD});
    }

    // The screen's own wallpaper, a copy of the actor GNOME draws it with (no new picture is loaded), enlarged a little
    // so that it can drift without showing its edge
    _picture(layer, monitor) {
        const frame = new St.Widget({
            style_class: 'amethystora-rest-screen',
            x: monitor.x, y: monitor.y, width: monitor.width, height: monitor.height,
            clip_to_allocation: true,
        });
        layer.add_child(frame);
        const source = Main.layoutManager._bgManagers?.[monitor.index]?.backgroundActor ?? null;
        let picture = null;
        if (source) {
            picture = new Clutter.Clone({source, width: monitor.width, height: monitor.height});
            picture.set_pivot_point(0.5, 0.5);
            picture.set_scale(DRIFT_SCALE, DRIFT_SCALE);
            frame.add_child(picture);
        }
        // A veil, so that the clock reads on any wallpaper
        frame.add_child(new St.Widget({
            style_class: 'amethystora-rest-veil', width: monitor.width, height: monitor.height,
        }));
        return {monitor, picture};
    }

    // On the main screen: the time, the day, what plays, and when it locks
    _face(layer) {
        const monitor = Main.layoutManager.primaryMonitor;
        if (!monitor)
            return;
        const face = new St.BoxLayout({
            style_class: 'amethystora-rest-face',
            orientation: Clutter.Orientation.VERTICAL,
            x_align: Clutter.ActorAlign.CENTER,
        });
        this._time = new St.Label({style_class: 'amethystora-rest-time', x_align: Clutter.ActorAlign.CENTER});
        this._day = new St.Label({style_class: 'amethystora-rest-day', x_align: Clutter.ActorAlign.CENTER});
        this._playing = new St.Label({style_class: 'amethystora-rest-playing', x_align: Clutter.ActorAlign.CENTER});
        this._playing.clutter_text.use_markup = false;
        this._playing.clutter_text.ellipsize = Pango.EllipsizeMode.END;
        this._note = new St.Label({style_class: 'amethystora-rest-note', x_align: Clutter.ActorAlign.CENTER});
        for (const label of [this._time, this._day, this._playing, this._note])
            face.add_child(label);
        layer.add_child(face);
        const place = () => face.set_position(
            Math.round(monitor.x + (monitor.width - face.width) / 2),
            Math.round(monitor.y + (monitor.height - face.height) / 2));
        face.connect('notify::width', place);
        face.connect('notify::height', place);
        place();
        this._lockAt = this._whenLocked();
    }

    // When the screen locks, as GNOME's settings make it: the session goes idle after idle-delay, the shield fades in,
    // and it locks after lock-delay (at least the fade). Null when nothing locks.
    _whenLocked() {
        const session = new Gio.Settings({schema_id: SESSION});
        const screensaver = new Gio.Settings({schema_id: SCREENSAVER});
        const delay = session.get_uint('idle-delay');
        const enabled = screensaver.get_boolean('lock-enabled');
        const lockDelay = screensaver.get_uint('lock-delay');
        session.run_dispose();
        screensaver.run_dispose();
        if (!delay || !enabled)
            return null;
        const left = delay * 1000 - this._monitor.get_idletime() + Math.max(SHIELD_FADE_MS, lockDelay * 1000);
        return GLib.DateTime.new_now_local().add_seconds(Math.max(0, left) / 1000);
    }

    // The clock and its words, set again at each turn of the minute
    _tick() {
        if (!this._layer)
            return;
        const now = GLib.DateTime.new_now_local();
        const clock = new Gio.Settings({schema_id: 'org.gnome.desktop.interface'});
        const twelve = clock.get_string('clock-format') === '12h';
        clock.run_dispose();
        if (this._time)
            this._time.text = now.format(twelve ? '%l:%M' : '%H:%M').trim();
        if (this._day)
            this._day.text = now.format('%A %-d %B');
        const media = this._ext.modules?.get('pill')?._media;
        if (this._playing) {
            this._playing.text = media?.playing ? media.text : '';
            this._playing.visible = !!this._playing.text;
        }
        if (this._note) {
            this._note.text = this._lockAt
                ? _('Locks at %s, as it would anyway · any key comes back').format(this._lockAt.format(twelve ? '%l:%M %p' : '%H:%M').trim())
                : _('Any key comes back');
        }
        if (this._tickId)
            GLib.source_remove(this._tickId);
        this._tickId = GLib.timeout_add_seconds(GLib.PRIORITY_DEFAULT, 60 - now.get_second(), () => {
            this._tickId = 0;
            this._tick();
            return GLib.SOURCE_REMOVE;
        });
    }

    // ---- The drift, only while it may move ----

    get _stillReason() {
        const comfort = this._ext.modules?.get('comfort');
        if (comfort?.reduceMotion || comfort?.lighter || !St.Settings.get().enable_animations)
            return 'calm';
        if (this._onBattery)
            return 'battery';
        return null;
    }

    _syncDrift() {
        if (!this._layer)
            return;
        if (!this._upower) {
            this._upowerCancellable = new Gio.Cancellable();
            Gio.DBusProxy.new_for_bus(Gio.BusType.SYSTEM, Gio.DBusProxyFlags.NONE, null, UPOWER, UPOWER_PATH, UPOWER,
                this._upowerCancellable, (source, result) => {
                    try {
                        this._upower = Gio.DBusProxy.new_for_bus_finish(result);
                    } catch {
                        return;
                    }
                    if (!this._layer)
                        return;
                    this._upower.connectObject('g-properties-changed', () => this._syncDrift(), this);
                    this._syncDrift();
                });
            this._unlistenComfort = this._ext.modules?.get('comfort')?.listen?.(() => this._syncDrift()) ?? null;
        }
        const value = this._upower?.get_cached_property('OnBattery');
        this._onBattery = value?.get_type_string() === 'b' ? value.unpack() : false;
        if (this._stillReason !== null) {
            this._stopDrift();
            return;
        }
        if (this._driftId)
            return;
        this._driftStart ??= GLib.get_monotonic_time();
        this._driftId = GLib.timeout_add(GLib.PRIORITY_LOW, Math.round(1000 / DRIFT_FPS), () => {
            this._drift();
            return GLib.SOURCE_CONTINUE;
        });
    }

    _stopDrift() {
        if (this._driftId)
            GLib.source_remove(this._driftId);
        this._driftId = 0;
    }

    // A slow sway on two axes, a little apart, within the room the enlargement leaves
    _drift() {
        const t = (GLib.get_monotonic_time() - this._driftStart) / 1e6;
        const phase = 2 * Math.PI * t / DRIFT_PERIOD_S;
        for (const {monitor, picture} of this._pictures ?? []) {
            if (!picture)
                continue;
            const roomX = monitor.width * (DRIFT_SCALE - 1) / 2;
            const roomY = monitor.height * (DRIFT_SCALE - 1) / 2;
            picture.set_translation(Math.round(roomX * 0.9 * Math.sin(phase)),
                Math.round(roomY * 0.9 * Math.sin(phase * 0.7 + 1)), 0);
        }
    }

    // ---- Back ----

    // byPerson: someone came back, and it fades; otherwise (a lock, the screens changed, the module stopping) at once
    _end(byPerson) {
        const layer = this._layer;
        if (!layer)
            return;
        this._layer = null;
        if (this._activeWatch)
            this._monitor.remove_watch(this._activeWatch);
        this._activeWatch = 0;
        if (this._tickId)
            GLib.source_remove(this._tickId);
        this._tickId = 0;
        this._stopDrift();
        this._driftStart = null;
        this._upowerCancellable?.cancel();
        this._upowerCancellable = null;
        this._upower?.disconnectObject(this);
        this._upower = null;
        this._unlistenComfort?.();
        this._unlistenComfort = null;
        Main.screenShield?.disconnectObject(this);
        Main.layoutManager.disconnectObject(this);
        if (this._grab) {
            Main.popModal(this._grab);
            this._grab = null;
        }
        this._pictures = null;
        this._time = this._day = this._playing = this._note = null;
        this._lockAt = null;
        if (byPerson && St.Settings.get().enable_animations) {
            layer.reactive = false;
            layer.ease({
                opacity: 0, duration: FADE_MS / 2, mode: Clutter.AnimationMode.EASE_OUT_QUAD,
                onStopped: () => layer.destroy(),
            });
        } else {
            layer.destroy();
        }
    }

    // For the suites and the harness: rest now, as after the time alone; and what it shows
    restNow() {
        if (!this._layer && this._why() === null)
            this._rest();
        return !!this._layer;
    }

    endNow() {
        this._end(false);
    }

    state() {
        return {
            enabled: this._settings?.get_boolean('enabled') ?? false,
            watching: !!this._idleWatch,
            resting: !!this._layer,
            why: this._why(),
            drifting: !!this._driftId,
            still: this._layer ? this._stillReason : null,
            time: this._time?.text ?? null,
            note: this._note?.text ?? null,
        };
    }
}
