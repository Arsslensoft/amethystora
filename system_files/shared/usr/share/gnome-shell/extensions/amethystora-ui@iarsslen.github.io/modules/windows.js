// Windows, for the classic layout and for every window whichever layout is on: snap layouts (drag to the top, or
// Super+Z), a window switcher you can type into, the shortcuts shown while Super is held, hot corners, show the desktop
// by clicking the wallpaper, touchpad gestures, windows that go back to their places for each arrangement of screens,
// a live copy of a window kept in view, dimming the windows not in use, the drop-down terminal and the scratchpad,
// windows kept on top that show it, see-through windows, a frame round the window in use, sizes for recording and
// opening again the app just closed. Each is a key of its own, read live; the parts that belong to the classic layout
// stand aside while the tiling is on and come back when it is off.
//
// The parts are in windows/: snap.js, switcher.js, hold.js, corners.js, desktop.js, gestures.js, places.js, keep.js,
// dim.js, scratch.js, ontop.js, seethrough.js, frame.js, sizes.js and reopen.js; menu.js puts their lines in the
// window menu. None of them runs anything while nothing happens: they listen, and only while they are on.

import GLib from 'gi://GLib';

import {Snap} from './windows/snap.js';
import {Switcher} from './windows/switcher.js';
import {Hold} from './windows/hold.js';
import {Corners, CORNERS} from './windows/corners.js';
import {Desktop} from './windows/desktop.js';
import {Gestures} from './windows/gestures.js';
import {Places} from './windows/places.js';
import {Keep} from './windows/keep.js';
import {Dim} from './windows/dim.js';
import {MenuHook} from './windows/menu.js';
import {Scratch} from './windows/scratch.js';
import {OnTop} from './windows/ontop.js';
import {SeeThrough} from './windows/seethrough.js';
import {Frame} from './windows/frame.js';
import {Sizes} from './windows/sizes.js';
import {Reopen} from './windows/reopen.js';

const GESTURES = ['pinch', 'spread', 'down', 'today'];
// In the order they start; they stop in the opposite one
const PARTS = ['snap', 'switcher', 'hold', 'desktop', 'corners', 'gestures', 'places', 'keep', 'dimmer',
    'scratch', 'ontop', 'seeThrough', 'frame', 'sizes', 'reopen'];

export class Windows {
    constructor(ext) {
        this._ext = ext;
    }

    get ext() {
        return this._ext;
    }

    enable() {
        this._counters = {};
        this.timings = {maxFrameUs: 0};
        this.settings = this._ext.getSettings('org.gnome.shell.extensions.amethystora-ui.windows');
        this._tiling = this._ext.getSettings('org.gnome.shell.extensions.amethystora-ui.tiling');
        // Every part exists while the module runs, so that asking one that is off says so rather than failing
        this.snap = new Snap(this);
        this.switcher = new Switcher(this);
        this.hold = new Hold(this);
        this.corners = new Corners(this);
        this.desktop = new Desktop(this);
        this.gestures = new Gestures(this);
        this.places = new Places(this);
        this.keep = new Keep(this);
        this.dimmer = new Dim(this);
        this.scratch = new Scratch(this);
        this.ontop = new OnTop(this);
        this.seeThrough = new SeeThrough(this);
        this.frame = new Frame(this);
        this.sizes = new Sizes(this);
        this.reopen = new Reopen(this);
        this._running = new Set();
        // The window menu's lines of the parts that run (a copy kept in view, the scratchpad, the sizes)
        this.menu = new MenuHook((menu, window) => ['keep', 'scratch', 'sizes']
            .filter(name => this._running?.has(name)).map(name => this[name].menuItems(menu, window)));
        this.menu.enable();
        this.settings.connectObject('changed', (s, key) => this._changed(key), this);
        // A change of layout is followed once the tiling has started or stopped itself, so that what it puts back
        // of GNOME's (the switchers' handlers) is not put over what this sets
        this._tiling.connectObject('changed::enabled', () => {
            this._layoutId ||= GLib.idle_add(GLib.PRIORITY_DEFAULT_IDLE, () => {
                this._layoutId = 0;
                this._sync();
                return GLib.SOURCE_REMOVE;
            });
        }, this);
        this._sync();
    }

    disable() {
        this.settings?.disconnectObject(this);
        this._tiling?.disconnectObject(this);
        if (this._layoutId)
            GLib.source_remove(this._layoutId);
        this._layoutId = 0;
        // In the opposite order of starting; places writes what it holds, the kept copy waits out a screen lock
        for (const name of [...PARTS].reverse())
            this._stop(name, true);
        this._running = null;
        this.menu?.disable();
        this.menu = null;
        // Disposed now rather than by the collector (dock.js says why)
        this.settings?.run_dispose();
        this.settings = null;
        this._tiling?.run_dispose();
        this._tiling = null;
    }

    // Whether the classic layout is on: the tiling's own switch off
    get classic() {
        return !this._tiling?.get_boolean('enabled');
    }

    // What should run now, part by part
    _wanted() {
        const s = this.settings;
        const classic = this.classic;
        return {
            snap: classic && s.get_boolean('snap'),
            switcher: classic && s.get_boolean('switcher'),
            hold: s.get_boolean('hold-super'),
            desktop: classic,
            corners: CORNERS.some(where => s.get_string(`corner-${where}`) !== 'none'),
            gestures: GESTURES.some(name => s.get_boolean(`gesture-${name}`)),
            places: classic && s.get_boolean('places'),
            keep: true,
            dimmer: s.get_boolean('dim'),
            scratch: s.get_boolean('drop-down') || classic && s.get_boolean('scratchpad'),
            ontop: s.get_boolean('keep-on-top'),
            seeThrough: s.get_boolean('see-through'),
            frame: classic && s.get_boolean('frame'),
            sizes: s.get_boolean('recording-sizes'),
            reopen: s.get_boolean('reopen'),
        };
    }

    _sync() {
        const wanted = this._wanted();
        for (const name of PARTS) {
            if (wanted[name])
                this._start(name);
            else
                this._stop(name, true);
        }
        // What the scratchpad holds follows the layout and its switches
        if (this._running.has('scratch'))
            this.scratch.sync();
        this.surfacesChanged();
    }

    _changed(key) {
        if (key.startsWith('corner-') && this._running.has('corners'))
            this.corners.rebuild();
        if (key === 'show-desktop') {
            this.desktop.syncClick();
            if (!this.settings.get_boolean('show-desktop'))
                this.desktop.bringBack();
        }
        // The card shows the snap key as it is set
        if (key === 'snap' || key === 'snap-chooser')
            this.hold?._stale();
        // Places turned off by the person drops what it holds and writes nothing more
        if (key === 'places' && !this.settings.get_boolean('places')) {
            this._stop('places', false);
            return;
        }
        // Reopening turned off forgets what closed
        if (key === 'reopen' && !this.settings.get_boolean('reopen'))
            this.reopen.forget();
        this._sync();
    }

    _start(name) {
        if (this._running.has(name))
            return;
        try {
            this[name].enable();
            this._running.add(name);
        } catch (e) {
            console.error(`Amethystora UI: the windows module's ${name} did not start: ${e.message}\n${e.stack}`);
            try {
                this[name].disable();
            } catch {}
        }
    }

    _stop(name, save) {
        if (!this._running?.has(name))
            return;
        this._running.delete(name);
        try {
            if (name === 'places')
                this[name].disable(save);
            else
                this[name].disable();
        } catch (e) {
            console.error(`Amethystora UI: the windows module's ${name} did not stop cleanly: ${e.message}\n${e.stack}`);
        }
    }

    running(name) {
        return this._running?.has(name) ?? false;
    }

    // --- for the other modules ------------------------------------------------------------------------------------

    // The motion module asks before it animates a minimize: a window kept in view goes out of sight its own way
    isKept(window) {
        return (this.keep?.isKept(window) ?? false) || this.holds(window);
    }

    // The drop-down terminal and the scratchpad's windows, which the shell itself keeps above and on every workspace
    holds(window) {
        return this._running?.has('scratch') ? this.scratch.holds(window) : false;
    }

    // Places asks before it puts a new window where its app's was: the drop-down terminal, and an app opened again by
    // Super+Alt+T, go where they go
    claims(window) {
        return (!!this._running?.has('scratch') && this.scratch.claims(window)) ||
            (!!this._running?.has('reopen') && this.reopen.claimed(window));
    }

    // The share guard covered or uncovered windows: a covered one is drawn solid
    coversChanged() {
        if (this._running?.has('seeThrough'))
            this.seeThrough.coversChanged();
    }

    // The scratchpad took or gave back a window: whether it has a ring follows
    heldChanged(window) {
        if (this._running?.has('ontop'))
            this.ontop.resync(window);
    }

    glassSurfaces() {
        if (!this._running)
            return [];
        return [this.snap, this.switcher, this.hold, this.desktop]
            .flatMap(part => part.glassSurfaces?.() ?? []);
    }

    surfacesChanged() {
        this._ext.modules?.get('glass')?.surfacesChanged?.();
    }

    // --- what it costs, for tests ---------------------------------------------------------------------------------

    count(name) {
        this._counters[name] = (this._counters[name] ?? 0) + 1;
    }

    // A handler's run, timed from t0 (GLib.get_monotonic_time())
    note(name, t0) {
        this.count(name);
        const spent = GLib.get_monotonic_time() - t0;
        if (spent > this.timings.maxFrameUs)
            this.timings.maxFrameUs = spent;
    }

    get counters() {
        return {...this._counters, 'hold.keymapEvents': this.hold?.keymapEvents ?? 0};
    }
}
