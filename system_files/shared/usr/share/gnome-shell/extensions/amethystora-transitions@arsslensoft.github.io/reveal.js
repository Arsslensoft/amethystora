// The reveal at login: the session starts the way the boot splash ends, its sky over every screen and the gem where the
// splash draws it, and then the gem flies into the top bar to become the logo menu's gem while the desktop shows
// through the sky and the top bar settles into place. Once per session start: not after unlocking, not when the
// extension is turned off and on, and not at all with animations off or the reveal key off.
//
// Only transforms and opacities of four actors change, frame by frame, from one timeline; nothing is laid out again
// while it runs. The sky never takes a click: GNOME's own start-up cover keeps input away while its animation runs,
// as it always has, and once that is over a click goes where it would. GNOME's own start-up ends in the overview, and
// the reveal is of the desktop, so the overview leaves first, unseen behind the sky.

import Clutter from 'gi://Clutter';
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import Graphene from 'gi://Graphene';
import St from 'gi://St';

import * as Main from 'resource:///org/gnome/shell/ui/main.js';

// Nominal; the shell's slow-down factor (0.6 in the image) is applied on top
const DURATION = 1200;
const GEM = '/usr/share/icons/hicolor/scalable/apps/amethystora-logo.svg';
const LOGO_MENU = 'amethystora-logo-menu@arsslensoft.github.io';
// The splash's plain black
const SKY = 'background-color: #000000;';
// Where and how large the splash draws the gem: its centre at 46 % of the screen's height, its height 280/1080 of it
const GEM_CENTRE = 0.46;
const GEM_HEIGHT = 280 / 1080;
const HALO = 1.5;
const SWELL = 1.06;
// How far above its place the top bar starts
const BAR_DROP = 6;
// The longest a test may hold the reveal at its start, in seconds
const HOLD_MOST = 60;

// A CSS cubic Bézier, as a function from time to progress, both 0..1
function cubicBezier(x1, y1, x2, y2) {
    const at = (a, b, s) => 3 * a * s * (1 - s) ** 2 + 3 * b * s * s * (1 - s) + s ** 3;
    const slope = (a, b, s) => 3 * a * (1 - s) ** 2 + 6 * (b - a) * s * (1 - s) + 3 * (1 - b) * s * s;
    return t => {
        if (t <= 0)
            return 0;
        if (t >= 1)
            return 1;
        // Newton's method from t itself, then halving the interval if it does not settle
        let s = t;
        for (let i = 0; i < 6; i++) {
            const error = at(x1, x2, s) - t;
            if (Math.abs(error) < 1e-5)
                return at(y1, y2, s);
            const d = slope(x1, x2, s);
            if (Math.abs(d) < 1e-6)
                break;
            s -= error / d;
        }
        let lo = 0, hi = 1;
        s = t;
        for (let i = 0; i < 30; i++) {
            const x = at(x1, x2, s);
            if (Math.abs(x - t) < 1e-5)
                break;
            if (x < t)
                lo = s;
            else
                hi = s;
            s = (lo + hi) / 2;
        }
        return at(y1, y2, s);
    };
}

const GLIDE = cubicBezier(0.5, 0, 0.2, 1);
const EASE = cubicBezier(0.25, 0.1, 0.25, 1);

// How far along a segment of the timeline p is, eased by its own curve
function segment(p, from, to, curve) {
    return curve(Math.min(1, Math.max(0, (p - from) / (to - from))));
}

const lerp = (a, b, t) => a + (b - a) * t;

export class Reveal {
    constructor(settings) {
        this._settings = settings;
        this._cover = null;
        this._panelMoved = false;
        // The longest a frame took in the handler, in microseconds, for the harness to read
        this.frameMax = 0;
    }

    // Builds the sky, the halo and the gem at once, when the session is starting; the animation itself waits for
    // GNOME's own start-up to finish. Returns whether there is a reveal.
    start() {
        if (!Main.layoutManager._startingUp || !this._settings.get_boolean('reveal') ||
            !St.Settings.get().enable_animations)
            return false;
        const monitor = Main.layoutManager.primaryMonitor;
        if (!monitor)
            return false;

        try {
            this._build(monitor);
        } catch (e) {
            this.stop();
            throw e;
        }
        return true;
    }

    _build(monitor) {
        const uiGroup = Main.layoutManager.uiGroup;
        const panelBox = Main.layoutManager.panelBox;

        this._cover = new St.Widget({style: SKY, reactive: false, name: 'amethystoraRevealCover'});
        this._cover.add_constraint(new Clutter.BindConstraint({
            source: global.stage,
            coordinate: Clutter.BindCoordinate.ALL,
        }));
        uiGroup.add_child(this._cover);

        const size = Math.round(monitor.height * GEM_HEIGHT);
        const halo = Math.round(size * HALO);
        const cx = monitor.x + monitor.width / 2;
        const cy = monitor.y + monitor.height * GEM_CENTRE;
        const [r, g, b] = this._accent();
        this._halo = new St.Widget({
            style: `background-gradient-direction: radial; background-color: rgba(${r}, ${g}, ${b}, 0.9); ` +
                `background-gradient-end: rgba(${r}, ${g}, ${b}, 0);`,
            reactive: false,
            width: halo,
            height: halo,
            x: Math.round(cx - halo / 2),
            y: Math.round(cy - halo / 2),
            pivot_point: new Graphene.Point({x: 0.5, y: 0.5}),
        });
        this._gem = new St.Icon({
            gicon: new Gio.FileIcon({file: Gio.File.new_for_path(GEM)}),
            icon_size: size,
            reactive: false,
            x: Math.round(cx - size / 2),
            y: Math.round(cy - size / 2),
            pivot_point: new Graphene.Point({x: 0.5, y: 0.5}),
        });
        uiGroup.add_child(this._halo);
        uiGroup.add_child(this._gem);
        // The top bar above the sky, so that it can come in on its own, hidden last; it goes back where it was at the end
        this._panelSibling = panelBox.get_previous_sibling();
        this._panelMoved = true;
        panelBox.set({opacity: 0, translation_y: -BAR_DROP});
        this._raise();

        // Chrome that comes after this (the dock, another extension's) would otherwise be drawn over the sky
        uiGroup.connectObject('child-added', () => this._raise(), this);
        Main.layoutManager.connectObject(
            'startup-complete', () => this._leaveOverview(),
            'monitors-changed', () => this.stop(),
            this);
    }

    // Sky on top of everything, then the top bar, the halo and the gem
    _raise() {
        const uiGroup = Main.layoutManager.uiGroup;
        const panelBox = Main.layoutManager.panelBox;
        for (const actor of [this._cover, panelBox, this._halo, this._gem]) {
            if (actor && actor.get_parent() === uiGroup)
                uiGroup.set_child_above_sibling(actor, null);
        }
    }

    _leaveOverview() {
        if (!Main.overview.visible) {
            this._play();
            return;
        }
        Main.overview.connectObject('hidden', () => this._play(), this);
        Main.overview.hide();
    }

    _play() {
        Main.overview.disconnectObject(this);
        Main.layoutManager.disconnectObject(this);
        Main.layoutManager.connectObject('monitors-changed', () => this.stop(), this);
        if (!this._cover)
            return;
        console.log('Amethystora Transitions: reveal');
        this._raise();
        this._timeline = new Clutter.Timeline({
            actor: this._cover,
            duration: Math.max(1, Math.round(DURATION * St.Settings.get().slow_down_factor)),
        });
        this._timeline.connectObject(
            'new-frame', () => this._frame(this._timeline.get_progress()),
            'stopped', () => this.stop(),
            this);
        // A test of the harness holds it at the start, to look at it frame by frame (seek, play)
        if (GLib.getenv('AMETHYSTORA_TEST')?.includes('reveal-hold')) {
            this._frame(0);
            // Held for a test at most a minute, whatever leaked the variable into a session
            this._holdId = GLib.timeout_add_seconds(GLib.PRIORITY_DEFAULT, HOLD_MOST, () => {
                this._holdId = 0;
                this.play();
                return GLib.SOURCE_REMOVE;
            });
        } else {
            this._timeline.start();
        }
    }

    // Test hook (AMETHYSTORA_TEST): the reveal as it is at p, held there
    seek(p) {
        if (!GLib.getenv('AMETHYSTORA_TEST') || !this._timeline)
            return false;
        this._timeline.pause();
        this._frame(p);
        return true;
    }

    play() {
        if (!this._timeline)
            return false;
        this._timeline.start();
        return true;
    }

    // Where the gem flies to: the logo menu's gem in the top bar, as it will be once the bar has settled
    _target() {
        const button = Main.panel.statusArea[LOGO_MENU];
        const icon = button?.get_children().find(child => child instanceof St.Icon);
        if (!icon || !icon.mapped)
            return null;
        // The icon may be taller than its picture (it fills the bar's height): the picture is the square in its middle
        const box = icon.get_transformed_extents();
        const size = Math.min(box.get_width(), box.get_height());
        if (size <= 0)
            return null;
        const centre = box.get_center();
        return {x: centre.x, y: centre.y - Main.layoutManager.panelBox.translation_y, size};
    }

    _frame(p) {
        const started = GLib.get_monotonic_time();
        const panelBox = Main.layoutManager.panelBox;

        // The gem swells in place, then flies; the halo swells with it and fades on the way
        const swell = segment(p, 0, 0.3, GLIDE);
        const fly = segment(p, 0.3, 1, GLIDE);
        if (p > 0.3 && this._flight === undefined)
            this._flight = this._flightPath();
        let scale = lerp(1, SWELL, swell), dx = 0, dy = 0, gemOpacity = 255;
        if (this._flight) {
            scale = lerp(SWELL, this._flight.scale, fly);
            dx = this._flight.dx * fly;
            dy = this._flight.dy * fly;
        } else if (p > 0.3) {
            // No logo menu in the bar: the gem shrinks where it is and fades
            scale = lerp(SWELL, 0.5, fly);
            gemOpacity = Math.round(255 * (1 - fly));
        }
        this._gem.set({scale_x: scale, scale_y: scale, translation_x: dx, translation_y: dy, opacity: gemOpacity});
        const haloScale = this._flight ? scale : lerp(1, SWELL, swell);
        this._halo.set({
            scale_x: haloScale,
            scale_y: haloScale,
            translation_x: dx,
            translation_y: dy,
            opacity: Math.round(255 * (1 - fly)),
        });

        // The desktop shows through the sky, then the top bar comes down into place
        this._cover.opacity = Math.round(255 * (1 - segment(p, 0.25, 0.8, EASE)));
        const bar = segment(p, 0.55, 1, EASE);
        panelBox.set({opacity: Math.round(255 * bar), translation_y: -BAR_DROP * (1 - bar)});

        this.frameMax = Math.max(this.frameMax, GLib.get_monotonic_time() - started);
    }

    // Read once, as the gem starts to fly
    _flightPath() {
        const target = this._target();
        if (!target)
            return null;
        const gem = this._gem;
        const cx = gem.x + gem.width / 2;
        const cy = gem.y + gem.height / 2;
        return {dx: target.x - cx, dy: target.y - cy, scale: target.size / Math.min(gem.width, gem.height)};
    }

    // At the end, or whatever cut it short (the extension turned off, a screen added or taken away, the screen
    // locked): the top bar is put back as it was before anything else, then the rest goes
    stop() {
        const layoutManager = Main.layoutManager;
        const panelBox = layoutManager.panelBox;
        // Only a top bar this hid and moved is put back: a start that failed before then left it as it was
        if (this._panelMoved) {
            this._panelMoved = false;
            panelBox.set({opacity: 255, translation_y: 0});
            const uiGroup = layoutManager.uiGroup;
            const sibling = this._panelSibling;
            if (panelBox.get_parent() === uiGroup) {
                if (sibling && sibling.get_parent() === uiGroup && sibling !== panelBox)
                    uiGroup.set_child_above_sibling(panelBox, sibling);
                else if (global.top_window_group.get_parent() === uiGroup)
                    uiGroup.set_child_below_sibling(panelBox, global.top_window_group);
            }
        }
        if (this._holdId)
            GLib.source_remove(this._holdId);
        this._holdId = 0;
        layoutManager.uiGroup.disconnectObject(this);
        layoutManager.disconnectObject(this);
        Main.overview.disconnectObject(this);
        if (this._timeline) {
            this._timeline.disconnectObject(this);
            this._timeline.stop();
            this._timeline = null;
        }
        for (const name of ['_gem', '_halo', '_cover']) {
            this[name]?.destroy();
            this[name] = null;
        }
        this._panelSibling = null;
        this._flight = undefined;
    }

    _accent() {
        try {
            const [color] = St.ThemeContext.get_for_stage(global.stage).get_accent_color();
            return [color.red, color.green, color.blue];
        } catch {
            return [145, 72, 240];
        }
    }
}
