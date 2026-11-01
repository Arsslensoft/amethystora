// A navigation session: from the first tiling key until Super (the key's main modifier) is let go, the keyboard
// is the tiling's. The selection moves and the strip scrolls with every key, a map of the strip shows the way, and
// the window chosen gets the keyboard at the end. Windows can be carried along meanwhile (Super+T) and put down
// on the workspace the session ends on. The live switcher is GNOME's window switcher, with the window chosen drawn
// where its strip would show it.

import Clutter from 'gi://Clutter';
import GLib from 'gi://GLib';
import GObject from 'gi://GObject';
import Meta from 'gi://Meta';
import Pango from 'gi://Pango';
import St from 'gi://St';

import {gettext as _} from 'resource:///org/gnome/shell/extensions/extension.js';
import * as AltTab from 'resource:///org/gnome/shell/ui/altTab.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';

import * as Layout from './layout.js';
import {config} from './config.js';
import {Open} from './engine.js';
import {lower} from './keys.js';
import {WorkspacePreview, activateQuietly} from './previews.js';

// The map waits this long after the first key that moves, so that a quick press does not flash it; its size and
// the dimming of the windows not chosen are the settings' (config.js), as is the speed of a drift
const MAP_DELAY = 200;
// The space between the windows in the map
const MAP_GAP = 8;
// How much of the strip the map keeps in view past the selection when it is wider than the screen
const MAP_CONTEXT = 500;
// The highlight in the map, and the map itself, glide this long to the window chosen
const MAP_GLIDE = 150;
// The screens a key that goes on past the strip goes to
const SIDES = {
    left: Meta.DisplayDirection.LEFT, right: Meta.DisplayDirection.RIGHT,
    above: Meta.DisplayDirection.UP, below: Meta.DisplayDirection.DOWN,
};
// The order modifiers rank in, for the one that ends a session
const RANKED = [Clutter.ModifierType.SUPER_MASK, Clutter.ModifierType.CONTROL_MASK,
    Clutter.ModifierType.MOD1_MASK, Clutter.ModifierType.SHIFT_MASK];
// The live switcher's keys: [backward, the floating windows only]
const LIVE_TAB = {
    'live-alt-tab': [false, false], 'live-alt-tab-backward': [true, false],
    'live-alt-tab-scratch': [false, true], 'live-alt-tab-scratch-backward': [true, true],
};

// The modifiers a key was pressed with, Super under its one name
function maskOf(binding, event) {
    let mask = binding?.get_mask?.() ?? 0;
    if (!mask && event)
        mask = event.get_state();
    return mask & Clutter.ModifierType.MOD4_MASK ? mask | Clutter.ModifierType.SUPER_MASK : mask;
}

// A column's window used last
function recent(column) {
    const mru = global.display.get_tab_list(Meta.TabList.NORMAL_ALL, null);
    const rank = window => mru.includes(window) ? mru.indexOf(window) : Infinity;
    return column.reduce((best, window) => rank(window) < rank(best) ? window : best, column[0]);
}

export class Sessions {
    constructor(tiling) {
        this.tiling = tiling;
        this.current = null;
    }

    begin(name, {event, binding} = {}) {
        const engine = this.tiling.engine;
        // The live switcher is a window switcher of its own, which holds the keyboard itself
        if (LIVE_TAB[name]) {
            this.end();
            const [backward, floatingOnly] = LIVE_TAB[name];
            const popup = new LiveSwitcher(this.tiling, floatingOnly);
            // GNOME's switcher ends when the highest of these is let go, as it does for its own keys
            const real = Clutter.ModifierType.SHIFT_MASK | Clutter.ModifierType.CONTROL_MASK |
                Clutter.ModifierType.MOD1_MASK | Clutter.ModifierType.MOD4_MASK;
            const mask = binding?.get_mask?.() || (event?.get_state() ?? 0) & real;
            if (!popup.show(backward, name, mask))
                popup.destroy();
            return;
        }
        if (!this.current) {
            const mask = maskOf(binding, event);
            const principal = RANKED.find(bit => mask & bit) ?? 0;
            const session = new Session(this, principal);
            if (!session.start())
                return;
            this.current = session;
            engine.session = session;
        }
        this.current.act(name, event);
    }

    // Returns the windows a quiet end let go, which no strip holds any more
    end(options) {
        const session = this.current;
        session?.end(options);
        return session?.released ?? [];
    }

    destroy() {
        this.current?.end({quiet: true});
    }

    ended(session) {
        if (this.current === session) {
            this.current = null;
            this.tiling.engine.session = null;
        }
    }
}

class Session {
    constructor(sessions, mask) {
        this._sessions = sessions;
        this._tiling = sessions.tiling;
        this._engine = this._tiling.engine;
        this._mask = mask;
        this.strip = this._engine.activeStrip();
        // The screen the session is on, which the workspace it ends on is shown on
        this._monitor = this.strip.monitor;
        this._carried = [];
        // What a quiet end let go of, for whoever cut the session short and keeps tiling
        this.released = [];
        this._veils = new Map();
    }

    start() {
        this._grabActor = new St.Widget({reactive: true, can_focus: true, width: 0, height: 0});
        Main.uiGroup.add_child(this._grabActor);
        this._grab = Main.pushModal(this._grabActor);
        this._grabActor.connectObject(
            'key-press-event', (actor, event) => this._keyPress(event),
            'key-release-event', (actor, event) => this._keyRelease(event),
            'button-press-event', () => this._click(),
            'scroll-event', (actor, event) => this._scroll(event),
            this);
        Main.overview.connectObject('showing', () => this.end(), 'hiding', () => this.end(), this);
        this._pauseAnimations();
        this._engine.updateBox(this.strip);
        // A key let go before the grab began: the session is over as soon as it starts
        this._check = GLib.idle_add(GLib.PRIORITY_DEFAULT, () => {
            this._check = 0;
            const [, , mods] = global.get_pointer();
            if (!this._held(mods))
                this.end();
            return GLib.SOURCE_REMOVE;
        });
        return true;
    }

    // GNOME's own window animations (a window minimized, closed or opened) wait until the session is over, so that
    // nothing moves on the screen but what the keys move. GNOME's check is still asked, for what it keeps track of.
    _pauseAnimations() {
        const wm = Main.wm;
        const original = wm._shouldAnimateActor;
        if (typeof original !== 'function' || Object.prototype.hasOwnProperty.call(wm, '_shouldAnimateActor'))
            return;
        wm._shouldAnimateActor = function (...args) {
            original.apply(this, args);
            return false;
        };
        this._resumeAnimations = () => delete wm._shouldAnimateActor;
    }

    _held(mods) {
        if (mods & Clutter.ModifierType.MOD4_MASK)
            mods |= Clutter.ModifierType.SUPER_MASK;
        return !this._mask || (mods & this._mask) !== 0;
    }

    // Does what a tiling key means inside the session
    act(name, event) {
        const actions = this._tiling.actions;
        if (name === 'take-window') {
            this._take();
            return;
        }
        if (name === 'drift-left' || name === 'drift-right') {
            this._drift(name === 'drift-left' ? 1 : -1, event);
            return;
        }
        // The live switcher takes over from the session
        if (LIVE_TAB[name]) {
            this.end();
            this._sessions.begin(name, {event});
            return;
        }
        const workspaceKey = {
            'switch-up-workspace': [-1, false], 'switch-down-workspace': [1, false],
            'switch-up-workspace-from-all-monitors': [-1, false], 'switch-down-workspace-from-all-monitors': [1, false],
            'move-up-workspace': [-1, true], 'move-down-workspace': [1, true],
            'move-previous-workspace': ['back', true], 'move-previous-workspace-backward': ['forward', true],
            'previous-workspace': ['back', false], 'previous-workspace-backward': ['forward', false],
        }[name];
        if (workspaceKey) {
            this._workspace(...workspaceKey);
            return;
        }
        if (actions.isSessionKey(name)) {
            if (actions.move(name, this.strip))
                this._moved();
            else if (this._beyond(actions.beyond(name)))
                return;
            if (actions.showsMap(name) && !this._preview)
                this._wantMap();
            return;
        }
        // Any other tiling key acts on the window chosen, without ending the session
        actions.run(name, {event});
        this._moved();
    }

    _keyPress(event) {
        const key = event.get_key_symbol();
        if (key === Clutter.KEY_Escape) {
            this.end();
            return Clutter.EVENT_STOP;
        }
        if (this._carried.length && this._carryKey(event))
            return Clutter.EVENT_STOP;
        const action = global.display.get_keybinding_action(event.get_key_code(), event.get_state());
        if (action === Meta.KeyBindingAction.MINIMIZE) {
            this.strip.selected?.minimize();
            return Clutter.EVENT_STOP;
        }
        const name = this._tiling.keys.actionFor(event);
        if (name) {
            if (this._tiling.actions.endsSession(name)) {
                this.end();
                this._tiling.actions.run(name, {event});
            } else {
                this.act(name, event);
            }
        }
        // Every other key waits until the session is over
        return Clutter.EVENT_STOP;
    }

    _keyRelease(event) {
        if (this._drifting && lower(event.get_key_symbol()) === this._drifting.key)
            this._stopDrift();
        const [, , mods] = global.get_pointer();
        if (!this._held(mods))
            this.end();
        return Clutter.EVENT_STOP;
    }

    // A click ends the session on the window under the pointer
    _click() {
        const [x, y] = global.get_pointer();
        for (const window of this.strip.windows) {
            const vis = this._engine.info(window)?.vis;
            if (vis && x >= vis.x && x < vis.x + vis.width && y >= vis.y && y < vis.y + vis.height) {
                this.strip.selected = window;
                break;
            }
        }
        this.end();
        return Clutter.EVENT_STOP;
    }

    _scroll(event) {
        const direction = event.get_scroll_direction();
        const back = direction === Clutter.ScrollDirection.UP || direction === Clutter.ScrollDirection.LEFT;
        const forward = direction === Clutter.ScrollDirection.DOWN || direction === Clutter.ScrollDirection.RIGHT;
        if (back || forward) {
            if (this._tiling.actions.move(back ? 'switch-left' : 'switch-right', this.strip))
                this._moved();
        }
        return Clutter.EVENT_STOP;
    }

    // A key with nowhere to go in the strip goes on to the next screen, or the next workspace; returns whether it did
    _beyond(where) {
        const [kind, side] = where?.split('-') ?? [];
        if (kind === 'workspace') {
            this._workspace(side === 'up' ? -1 : 1, false);
            return true;
        }
        if (kind === 'monitor') {
            const to = global.display.get_monitor_neighbor_index(this.strip.monitor, SIDES[side]);
            if (to < 0)
                return false;
            this._toScreen(to, side);
            return true;
        }
        return false;
    }

    // The session goes on in the strip of the screen that way: at its first column going right, its last going
    // left, and up or down at the bottom or the top of the column in the same place. The pointer goes along, and
    // the map is drawn there.
    _toScreen(to, side) {
        const from = this.strip;
        const at = from.selected ? from.find(from.selected) : null;
        const strip = this._engine.activeStrip(to);
        const columns = strip.columns;
        let target = null;
        if (columns.length && side === 'right') {
            target = recent(columns[0]);
        } else if (columns.length && side === 'left') {
            target = recent(columns.at(-1));
        } else if (columns.length) {
            const column = columns[Math.min(at?.[0] ?? 0, columns.length - 1)];
            target = side === 'above' ? column.at(-1) : column[0];
        }
        const mapShown = !!this._map;
        this._closeMap();
        this.strip = strip;
        this._monitor = to;
        if (target) {
            strip.selected = target;
            this._engine.ensureVisible(strip, target);
        }
        this._engine.updateBox(from);
        this._tiling.actions.warpTo(to);
        if (this._carried.length)
            this._pile();
        this._moved();
        if (this._preview)
            return;
        if (mapShown)
            this._showMap();
        else
            this._wantMap();
    }

    _moved() {
        this._engine.updateBox(this.strip);
        this._map?.update();
        this._syncVeils();
        this._tiling.topBar?.positionsChanged();
    }

    // The strip moved under the session: the dimming moves with its windows, frame by frame
    follow(strip) {
        if (strip !== this.strip || !this._veils.size)
            return;
        for (const [window, veil] of this._veils) {
            const vis = this._engine.info(window)?.vis;
            if (vis) {
                veil.set_position(vis.x, vis.y);
                veil.set_size(vis.width, vis.height);
            }
        }
    }

    // ---- The map ----

    _wantMap() {
        if (this._map || this._mapTimer || config.mapScale <= 0)
            return;
        this._mapTimer = GLib.timeout_add(GLib.PRIORITY_DEFAULT, MAP_DELAY, () => {
            this._mapTimer = 0;
            this._showMap();
            return GLib.SOURCE_REMOVE;
        });
    }

    _showMap() {
        if (config.mapScale <= 0)
            return;
        this._map = new StripMap(this._engine, this.strip);
        this._syncVeils();
    }

    _closeMap() {
        if (this._mapTimer)
            GLib.source_remove(this._mapTimer);
        this._mapTimer = 0;
        this._map?.destroy();
        this._map = null;
    }

    // Every window of the strip on screen but the chosen one goes dark while the map is up
    _syncVeils() {
        const wanted = new Set(this._map && config.mapShade > 0 ? this.strip.windows.filter(w =>
            w !== this.strip.selected && !this._engine.info(w)?.hidden) : []);
        for (const [window, veil] of this._veils) {
            if (!wanted.has(window)) {
                this._veils.delete(window);
                veil.ease({opacity: 0, duration: config.animation, onStopped: () => veil.destroy()});
            }
        }
        for (const window of wanted) {
            let veil = this._veils.get(window);
            if (!veil) {
                veil = new St.Widget({style_class: 'amethystora-tiling-veil', opacity: 0, reactive: false});
                veil.set_style(`background-color: rgba(0, 0, 0, ${(0.7 * config.mapShade / 255).toFixed(3)});`);
                global.window_group.add_child(veil);
                this._veils.set(window, veil);
                veil.ease({opacity: 255, duration: config.animation});
            }
            const vis = this._engine.info(window).vis;
            const actor = window.get_compositor_private();
            veil.set_position(vis.x, vis.y);
            veil.set_size(vis.width, vis.height);
            if (actor && veil.get_parent() === actor.get_parent())
                global.window_group.set_child_above_sibling(veil, actor);
        }
    }

    // ---- Drifting ----

    _drift(direction, event) {
        if (this._drifting)
            return;
        const key = event ? lower(event.get_key_symbol()) : 0;
        const timeline = new Clutter.Timeline({actor: global.stage, duration: 1000, repeat_count: -1});
        let last = 0;
        timeline.connect('new-frame', (t, elapsed) => {
            const ms = elapsed >= last ? elapsed - last : elapsed;
            last = elapsed;
            const geo = this._engine.scrollTo(this.strip, this.strip.scroll + direction * config.driftSpeed * ms);
            this._followDrift(geo, direction);
        });
        this._drifting = {timeline, key};
        timeline.start();
    }

    // The selection follows the window most on screen in the direction the strip moves
    _followDrift(geo, direction) {
        const area = geo.area;
        let best = null, bestShown = 0;
        for (const window of this.strip.windows) {
            const rect = geo.rects.get(window);
            const shown = Math.min(rect.x + rect.width, area.x + area.width) - Math.max(rect.x, area.x);
            if (shown > bestShown || (shown === bestShown && shown > 0 && direction < 0)) {
                best = window;
                bestShown = shown;
            }
        }
        if (best && best !== this.strip.selected) {
            this.strip.selected = best;
            this._moved();
        }
    }

    // `quiet`: the session is being cut short, and the strip is not to be laid out again
    _stopDrift(quiet = false) {
        this._drifting?.timeline.stop();
        this._drifting = null;
        if (!quiet && this.strip.selected)
            this._engine.ensureVisible(this.strip, this.strip.selected);
    }

    // ---- Workspaces ----

    // The workspace keys open the cards of the workspaces (previews.js) and move along them; the one chosen is
    // shown when the session ends
    _workspace(step, carry) {
        if (carry && !this._carried.length)
            this._take();
        const pile = step === 'back' || step === 'forward';
        if (!this._preview) {
            // With a workspace on each screen, the cards are the ones of this screen and those shown nowhere
            const list = this._engine.screens.workspacesFor(this._monitor, pile, this._tiling.workspaceHistory());
            this._closeMap();
            this._preview = new WorkspacePreview(this._engine, this._monitor, pile ? 'pile' : 'row', list,
                Math.max(0, list.indexOf(this.strip.workspace)), this._carried);
        }
        let direction = step;
        if (pile)
            direction = step === 'back' ? 1 : -1;
        this._preview.step(direction);
        this.strip = this._engine.strip(this._preview.workspace, this._monitor);
        this._syncVeils();
    }

    // ---- Carrying ----

    _take() {
        const window = this.strip.selected;
        if (!window || this._carried.includes(window) || !this._engine.isTiled(window))
            return;
        const order = this.strip.windows;
        const index = order.indexOf(window);
        this._engine.remove(window);
        this.strip.selected = order[index + 1] ?? order[index - 1] ?? null;
        window.stick();
        window.make_above();
        this._carried.push(window);
        this._pile();
        this._hint();
        this._moved();
    }

    // The carried windows wait at the lower right of the screen, fanned out, the newest under the others
    _pile() {
        const monitor = Main.layoutManager.monitors[this.strip.monitor];
        this._carried.forEach((window, i) => {
            const n = i + 1;
            const frame = window.get_frame_rect();
            const to = {
                x: Math.round(monitor.x + monitor.width - 0.08 * monitor.width * (n + 1)),
                y: Math.round(monitor.y + monitor.height * 2 / 3 + 16 * n),
                width: frame.width,
                height: frame.height,
            };
            this._engine.glideFloating(window, this._engine.visual(window), to);
        });
        for (const window of this._carried)
            window.lower();
    }

    _hint() {
        if (!this._carried.length) {
            const hint = this._hintLabel;
            this._hintLabel = null;
            hint?.ease({opacity: 0, duration: config.animation, onStopped: () => hint.destroy()});
            return;
        }
        if (this._hintLabel)
            return;
        const monitor = Main.layoutManager.monitors[this.strip.monitor];
        const label = new St.Label({
            style_class: 'amethystora-tiling-hint', opacity: 0,
            text: _('Space puts the window down here, Tab goes through the windows you carry, Q closes them'),
        });
        label.clutter_text.line_wrap = true;
        label.clutter_text.ellipsize = Pango.EllipsizeMode.NONE;
        Main.uiGroup.add_child(label);
        label.set_position(monitor.x + monitor.width - 402, monitor.y + monitor.height - 100);
        label.width = 380;
        label.ease({opacity: 255, duration: config.animation});
        this._hintLabel = label;
    }

    _carryKey(event) {
        const key = event.get_key_symbol();
        const shift = event.get_state() & Clutter.ModifierType.SHIFT_MASK;
        if (key === Clutter.KEY_space) {
            this._putDown(this._carried.pop());
        } else if (key === Clutter.KEY_Tab || key === Clutter.KEY_ISO_Left_Tab) {
            if (shift || key === Clutter.KEY_ISO_Left_Tab)
                this._carried.push(this._carried.shift());
            else
                this._carried.unshift(this._carried.pop());
            this._pile();
        } else if (key === Clutter.KEY_q || key === Clutter.KEY_Q) {
            const carried = this._carried.splice(0);
            for (const window of carried) {
                this._putDown(window);
                window.delete(global.get_current_time());
            }
        } else {
            return false;
        }
        this._hint();
        return true;
    }

    _putDown(window) {
        if (!window)
            return;
        window.unmake_above();
        window.unstick();
        window.change_workspace(this.strip.workspace);
        const position = this._tiling.openPosition;
        this._engine.insert(window, this.strip, {position: position === Open.DOWN ? Open.DOWN : position});
        this._pile();
        this._moved();
    }

    // ---- The end ----

    end({quiet = false} = {}) {
        if (this._ended)
            return;
        this._ended = true;
        // The grab goes first, before anything that reads the strip or the screens, which may have changed under
        // the session (a monitor unplugged): a throw with the grab still held would leave every key and click going
        // to an actor nobody sees, and the ended flag keeps the session from trying again
        Main.overview.disconnectObject(this);
        this._grabActor.disconnectObject(this);
        try {
            Main.popModal(this._grab);
        } catch (e) {
            // Already let go by someone else: the rest of the end is still due
            console.warn(`Amethystora UI: the navigation's grab was not released: ${e.message}`);
        }
        this._grabActor.destroy();
        if (this._check)
            GLib.source_remove(this._check);
        this._check = 0;
        this._resumeAnimations?.();
        this._resumeAnimations = null;
        // One step failing does not keep the others from running
        const steps = [() => this._stopDrift(quiet), () => this._closeMap(), () => this._syncVeils()];
        for (const step of steps) {
            try {
                step();
            } catch (e) {
                console.error(`Amethystora UI: the tiling's navigation did not end cleanly: ${e.message}\n${e.stack}`);
            }
        }
        this._sessions.ended(this);
        const preview = this._preview;
        this._preview = null;
        if (preview && !quiet)
            preview.finish(workspace => this._finish(workspace, false));
        else
            preview?.destroy();
        if (!preview || quiet)
            this._finish(this.strip.workspace, quiet);
    }

    // The workspace chosen is shown, what is carried is put down on it side by side, and the window chosen (or the
    // first put down) gets the keyboard
    _finish(workspace, quiet) {
        // The cards zoomed after the tiling stopped (the screen locked): there is no strip left to put anything in
        if (this._tiling.engine !== this._engine)
            quiet = true;
        if (!quiet)
            activateQuietly(this._engine, workspace, this._monitor);
        const carried = this._carried.splice(0);
        let first = null;
        for (const window of carried.reverse()) {
            // The tiling is stopping: the windows are only let go where they are
            if (quiet) {
                window.unmake_above();
                window.unstick();
                this.released.push(window);
                continue;
            }
            this._putDown(window);
            first ??= window;
        }
        this._hint();
        if (quiet)
            return;
        if (first)
            this.strip.selected = first;
        this._engine.focus(this.strip);
        this._engine.updateBox(this.strip);
        this._tiling.noteWorkspace(this.strip.workspace);
    }
}

// A small picture of the strip in the middle of the screen, in the look of GNOME's window switcher
class StripMap {
    constructor(engine, strip) {
        this._engine = engine;
        this._strip = strip;
        const monitor = Main.layoutManager.monitors[strip.monitor];
        this._monitor = monitor;
        this.actor = new St.BoxLayout({
            style_class: 'switcher-list amethystora-tiling-map',
            orientation: Clutter.Orientation.VERTICAL, opacity: 0,
        });
        this._view = new St.Widget({clip_to_allocation: true});
        this._content = new St.Widget();
        this._view.add_child(this._content);
        // The title of the window chosen, under its picture
        this._label = new St.Label({style_class: 'amethystora-tiling-map-title', x_align: Clutter.ActorAlign.START});
        this._label.clutter_text.ellipsize = Pango.EllipsizeMode.END;
        this.actor.add_child(this._view);
        this.actor.add_child(this._label);
        Main.uiGroup.add_child(this.actor);
        this._clones = new Map();
        this.update();
        this.actor.ease({opacity: 255, duration: config.animation, mode: Clutter.AnimationMode.EASE_OUT_EXPO});
        global.display.connectObject('restacked', () => this.update(), this);
    }

    // Where each window is drawn in the map: the strip at a small scale, with real gaps between the windows so
    // that the highlight shows around the chosen one
    _places(geo) {
        const places = new Map();
        let x = MAP_GAP;
        let bottom = 0;
        this._strip.columns.forEach((column, c) => {
            const width = Math.round(geo.columns[c].width * config.mapScale);
            let y = MAP_GAP;
            for (const window of column) {
                const height = Math.round(geo.rects.get(window).height * config.mapScale);
                places.set(window, {x, y, width, height});
                y += height + MAP_GAP;
            }
            bottom = Math.max(bottom, y);
            x += width + MAP_GAP;
        });
        return {places, width: x, height: Math.max(bottom, MAP_GAP * 2)};
    }

    update() {
        const strip = this._strip;
        const geo = this._engine.geometry(strip);
        const alive = new Set(strip.windows);
        for (const [window, item] of this._clones) {
            if (!alive.has(window)) {
                item.destroy();
                this._clones.delete(window);
            }
        }
        if (!this._highlight) {
            this._highlight = new St.Widget({style_class: 'item-box'});
            this._highlight.add_style_pseudo_class('selected');
            this._content.add_child(this._highlight);
        }
        const highlight = this._highlight;
        const {places, width, height} = this._places(geo);
        for (const window of strip.windows) {
            let item = this._clones.get(window);
            if (!item) {
                const actor = window.get_compositor_private();
                if (!actor)
                    continue;
                item = new Clutter.Clone({source: actor});
                this._content.add_child(item);
                this._clones.set(window, item);
            }
            // The clone draws the whole window actor, its shadows too: its frame is put where the map has it
            const place = places.get(window);
            const buffer = window.get_buffer_rect();
            const frame = window.get_frame_rect();
            const sx = place.width / Math.max(1, frame.width);
            const sy = place.height / Math.max(1, frame.height);
            item.set_scale(sx, sy);
            item.set_position(Math.round(place.x + (buffer.x - frame.x) * sx),
                Math.round(place.y + (buffer.y - frame.y) * sy));
        }
        this._content.set_size(width, height);
        const viewWidth = Math.min(width, this._monitor.width - 2 * 60);
        this._view.set_size(viewWidth, height);
        const selected = strip.selected ? places.get(strip.selected) : null;
        if (selected) {
            const pad = MAP_GAP / 2;
            // The highlight glides from the window it was on to the one chosen, once it is shown
            const box = {
                x: selected.x - pad, y: selected.y - pad,
                width: selected.width + 2 * pad, height: selected.height + 2 * pad,
            };
            if (highlight.visible && this._placed)
                highlight.ease({...box, duration: MAP_GLIDE, mode: Clutter.AnimationMode.EASE_OUT_QUAD});
            else
                highlight.set({...box});
            this._placed = true;
            highlight.show();
            this._content.set_child_below_sibling(highlight, null);
            this._label.text = strip.selected.get_title() ?? '';
            // A map wider than the screen keeps some of the strip beyond the selection in view
            let offset = this._offset ?? 0;
            if (width > viewWidth) {
                const context = Math.round(MAP_CONTEXT * config.mapScale);
                if (selected.x + selected.width + context > offset + viewWidth)
                    offset = selected.x + selected.width + context - viewWidth;
                if (selected.x - context < offset)
                    offset = selected.x - context;
                offset = Math.max(0, Math.min(offset, width - viewWidth));
            } else {
                offset = 0;
            }
            const first = this._offset === undefined;
            if (Math.round(offset) !== this._offset) {
                this._content.ease({
                    translation_x: -Math.round(offset), duration: first ? 0 : MAP_GLIDE,
                    mode: Clutter.AnimationMode.EASE_OUT_QUAD,
                });
                this._offset = Math.round(offset);
            }
            // Centred under the picture of the window, and kept inside the map
            this._label.width = -1;
            const [, natural] = this._label.get_preferred_width(-1);
            const shown = Math.min(natural, viewWidth);
            this._label.width = shown;
            const middle = selected.x + selected.width / 2 - this._offset;
            const x = Math.round(Math.max(0, Math.min(viewWidth - shown, middle - shown / 2)));
            this._label.ease({translation_x: x, duration: first ? 0 : MAP_GLIDE, mode: Clutter.AnimationMode.EASE_OUT_QUAD});
        } else {
            highlight.hide();
            this._label.text = '';
        }
        const [, natWidth] = this.actor.get_preferred_width(-1);
        const [, natHeight] = this.actor.get_preferred_height(natWidth);
        this.actor.set_position(Math.round(this._monitor.x + (this._monitor.width - natWidth) / 2),
            Math.round(this._monitor.y + (this._monitor.height - natHeight) / 2));
    }

    destroy() {
        global.display.disconnectObject(this);
        this.actor.destroy();
        this._clones.clear();
    }
}

// Where a tiled window would be drawn once its strip has scrolled to it, by the workspace's focus mode, without
// scrolling the strip
function scrolledTo(engine, strip, window) {
    // geometry() may centre the strip for the window asked about: it is left as it was
    const {selected, scroll, loose} = strip;
    strip.selected = window;
    try {
        const geo = engine.geometry(strip);
        const rect = geo.rects.get(window);
        const at = strip.find(window);
        if (!rect || !at || geo.centred || window.is_fullscreen())
            return rect ?? null;
        const column = geo.columns[at[0]];
        const now = geo.scroll + column.x;
        const x = Layout.placeFocused(geo.area, column, now, strip.mode, at[0], geo.columns.length);
        return {...rect, x: rect.x + Math.round(x - now)};
    } finally {
        Object.assign(strip, {selected, scroll, loose});
    }
}

// GNOME's window switcher over the windows of every workspace (or of this one, when GNOME's switcher is set to the
// current workspace only), the tiled ones first and the floating ones after them, or before them when a floating
// one has the keyboard; or the floating ones alone. The window chosen is drawn live where its strip would scroll it,
// over a fog on the rest of the screen, and gets the keyboard when the keys are let go; Escape leaves everything
// as it was.
const LiveSwitcher = GObject.registerClass(
class LiveSwitcher extends AltTab.WindowSwitcherPopup {
    _init(tiling, floatingOnly) {
        // GNOME's switcher asks for its list of windows while it is made
        this._tiling = tiling;
        this._floatingOnly = floatingOnly;
        super._init();
        this.connect('destroy', () => this._unlive());
    }

    _getWindowList() {
        const windows = super._getWindowList();
        const floating = this._tiling.engine?.floating ?? new Set();
        const above = windows.filter(window => floating.has(window));
        if (this._floatingOnly)
            return above;
        const tiled = windows.filter(window => !floating.has(window));
        return floating.has(global.display.focus_window) ? [...above, ...tiled] : [...tiled, ...above];
    }

    // The live switcher's own keys go on through the windows, as GNOME's go through its own
    vfunc_key_press_event(event) {
        const name = this._tiling.keys?.actionFor(event);
        const tab = LIVE_TAB[name];
        if (!tab)
            return super.vfunc_key_press_event(event);
        this._disableHover();
        this._select(tab[0] ? this._previous() : this._next());
        this._showImmediately();
        return Clutter.EVENT_STOP;
    }

    _select(num) {
        super._select(num);
        if (this._live)
            this._draw(this._items[num]?.window);
    }

    // Drawn only once the switcher shows, so that a quick press and release does not flash the screen
    _showImmediately() {
        super._showImmediately();
        if (!this._live) {
            this._live = true;
            this._draw(this._items[this._selectedIndex]?.window);
        }
    }

    _draw(window) {
        this._clone?.destroy();
        this._clone = null;
        const engine = this._tiling.engine;
        const actor = window?.get_compositor_private();
        if (!engine || !actor)
            return;
        const strip = engine.stripOf(window);
        const area = engine.area(strip?.monitor ?? window.get_monitor());
        if (!this._fog) {
            this._fog = new St.Widget({style_class: 'amethystora-tiling-fog', reactive: false});
            Main.uiGroup.insert_child_below(this._fog, this);
        }
        this._fog.set_position(area.x, area.y);
        this._fog.set_size(area.width, area.height);
        const rect = (strip && scrolledTo(engine, strip, window)) ?? engine.visual(window);
        const frame = window.get_frame_rect();
        const buffer = window.get_buffer_rect();
        this._clone = new Clutter.Clone({source: actor, reactive: false});
        this._clone.set_position(rect.x + buffer.x - frame.x, rect.y + buffer.y - frame.y);
        Main.uiGroup.insert_child_above(this._clone, this._fog);
    }

    _unlive() {
        this._clone?.destroy();
        this._fog?.destroy();
        this._clone = null;
        this._fog = null;
    }
});
