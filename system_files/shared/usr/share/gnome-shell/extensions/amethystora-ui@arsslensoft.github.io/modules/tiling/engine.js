// The strips: which windows tile, in what order, where each one is, and how they glide there.
//
// A window's place on screen is where the strip says, always. GNOME lets a window go only so far past the edge
// of the screen, so the frame goes as close as GNOME allows and the window actor's translation makes up the
// rest; a window that is entirely off its monitor is hidden. Gliding moves that drawn place from where it was to
// where it belongs over the animation time, with the frame already at the end, so that GNOME's own size and
// position never lag behind more than one move.

import Clutter from 'gi://Clutter';
import GLib from 'gi://GLib';
import GObject from 'gi://GObject';
import Meta from 'gi://Meta';
import St from 'gi://St';

import {gettext as _} from 'resource:///org/gnome/shell/extensions/extension.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import {adjustAnimationTime} from 'resource:///org/gnome/shell/misc/animationUtils.js';

import * as Layout from './layout.js';
import {config, forget, preferredWidth, ruleFor} from './config.js';
import {Screens} from './screens.js';
import {WorkspaceBackgrounds} from './backgrounds.js';

// How long an app has to answer a new size: a size it reports later than that is its own choice, which the strip
// takes at once; one sooner may be its answer to an older request
const ANSWER = 120;
// The least of a window GNOME keeps on screen, with a little room
const ONSCREEN = 80;
// The focus box's opacity while its window does not have the keyboard
const INACTIVE = 140;
// How long a new window waits beside the focused one before it is stacked under it, and a window that opens
// fullscreen before it takes the screen again, in milliseconds
const PULL_IN = 100;
const REFULLSCREEN = 100;
// Where new windows open, the values of open-window-position
export const Open = {RIGHT: 0, LEFT: 1, START: 2, END: 3, DOWN: 4, UP: 5};

// The effect a window cut at its screen's edge carries; it draws nothing of its own
const CUT = 'amethystora-tiling-cut';
const Uncovered = GObject.registerClass(class Uncovered extends Clutter.Effect {});

// What survives the tiling being stopped and started again in the same session: at every screen lock, and when
// it is switched off and on
const remembered = {strips: [], windows: new WeakMap(), floating: new Set()};

// GNOME's previewer (Space in Files, and Glance) belongs over the file's window, never in a strip. A window gets no
// signal when it gains a parent, and Glance's has none, so it goes by its name
const PREVIEWER = 'org.gnome.NautilusPreviewer';
const isPreviewer = window => window.get_sandboxed_app_id() === PREVIEWER || window.get_wm_class() === PREVIEWER;

// Which screen each screen number is, by its connector. The numbers shift when a screen other than the last is
// unplugged, so a strip follows its screen by this and not by its number.
function connectors() {
    const list = [];
    for (const logical of global.backend.get_monitor_manager().get_logical_monitors())
        list[logical.get_number()] = logical.get_monitors()[0]?.get_connector() ?? null;
    return list;
}

export class Strip {
    constructor(workspace, monitor) {
        this.workspace = workspace;
        this.monitor = monitor;
        // Each column is its windows from the top
        this.columns = [];
        // The screen x of the strip's left end, from the left of the work area
        this.scroll = 0;
        // Moved aside from the middle while narrower than the screen (geometry)
        this.loose = false;
        this.mode = config.defaultMode;
        // The window the strip shows as focused, the keyboard's or the one a navigation is on
        this.selected = null;
        // While centring: which half the window was in before, to put it back there
        this.side = 0;
    }

    get windows() {
        return this.columns.flat();
    }

    find(window) {
        for (let c = 0; c < this.columns.length; c++) {
            const r = this.columns[c].indexOf(window);
            if (r >= 0)
                return [c, r];
        }
        return null;
    }
}

export class Engine {
    constructor(tiling) {
        this.tiling = tiling;
    }

    enable() {
        this._strips = new Map();
        this._info = new Map();
        this.floating = new Set();
        this._boxes = new Map();
        this._glides = new Map();
        this.session = null;
        this.dragging = null;
        this._connectors = connectors();
        // Something drawn over the strips that the focus box gives way to
        this.boxHidden = 0;

        const display = global.display;
        display.connectObject(
            'window-created', (d, window) => this._created(window),
            'notify::focus-window', () => this._focusChanged(),
            'grab-op-begin', (d, window, op) => this._grabBegan(window, op),
            'grab-op-end', (d, window) => this._grabEnded(window),
            'workareas-changed', () => this.layoutAll(false),
            'restacked', () => this._restackBoxes(),
            this);
        // After everyone else's handlers, GNOME's and the motion's, which say whether the window is opening
        global.window_manager.connectObject(
            'map', (wm, actor) => this._boxFollows(actor), GObject.ConnectFlags.AFTER,
            'unminimize', (wm, actor) => this._boxFollows(actor), GObject.ConnectFlags.AFTER,
            this);
        global.workspace_manager.connectObject(
            'active-workspace-changed', () => this._workspaceChanged(),
            'workspace-removed', () => this._prune(),
            this);
        Main.layoutManager.connectObject('monitors-changed', () => this._monitorsChanged(), this);
        Main.overview.connectObject('hidden', () => this._overviewHidden(), this);

        // Which workspace each screen shows, when there are several (screens.js)
        this.screens = new Screens(this);
        this.screens.enable();
        // Each workspace's own picture, on the screen that shows it (backgrounds.js)
        this.backgrounds = new WorkspaceBackgrounds(this);
        this.backgrounds.enable();
        // GNOME's workspaces on the main screen only (the classic layout) keep the other screens' windows on every
        // workspace. Once settings.js has turned that off, mutter puts each of them on GNOME's workspace, later, on
        // idle: each goes to the workspace its own screen shows instead (_stickyChanged), or it would be drawn there
        // only while the screen with the keyboard showed its workspace
        this._unpinned = new WeakSet(global.display.list_all_windows().filter(window =>
            window.is_on_all_workspaces() && window.get_monitor() !== Main.layoutManager.primaryIndex));
        this._restore();
        this.screens.assign(this.tiling.workspaceHistory?.() ?? []);
    }

    // suspend: the screen is locking. What the strips are is kept for unlock, and everything drawn is put back.
    disable(suspend = false) {
        global.display.disconnectObject(this);
        global.window_manager.disconnectObject(this);
        global.workspace_manager.disconnectObject(this);
        Main.layoutManager.disconnectObject(this);
        Main.overview.disconnectObject(this);
        this.screens?.disable();
        this.screens = null;
        this.backgrounds?.disable();
        this.backgrounds = null;
        for (const glide of this._glides?.values() ?? [])
            glide.timeline.stop();
        this._glides = null;
        this._remember();
        for (const box of this._boxes?.values() ?? [])
            box.destroy();
        this._boxes = null;
        if (!suspend) {
            for (const window of this.floating ?? [])
                this._unfloatFlags(window);
        }
        for (const [window, info] of [...this._info ?? []]) {
            this._untrack(window);
            if (!suspend)
                this._release(window, info);
            else
                this._unhide(window, info);
        }
        this._info = null;
        this._strips = null;
        this.floating = null;
        this._unpinned = null;
    }

    // ---- The model ----

    info(window) {
        return this._info.get(window) ?? null;
    }

    stripOf(window) {
        return this._info.get(window)?.strip ?? null;
    }

    // A workspace's strip on a screen. With one screen a workspace has one strip; with several, each screen showing a
    // workspace of its own, a workspace still has one, on the screen it is shown on or was last shown on.
    strip(workspace, monitor) {
        let byMonitor = this._strips.get(workspace);
        if (!byMonitor) {
            byMonitor = new Map();
            this._strips.set(workspace, byMonitor);
        }
        if (this.screens?.separate) {
            const own = byMonitor.values().next().value;
            if (own)
                return own;
            monitor = this.screens.placeOf(workspace, monitor);
        }
        let strip = byMonitor.get(monitor);
        if (!strip) {
            strip = new Strip(workspace, monitor);
            byMonitor.set(monitor, strip);
        }
        return strip;
    }

    // A strip taken to another screen with its workspace; its windows go there at the next layout
    moveStrip(strip, monitor) {
        const byMonitor = this._strips.get(strip.workspace);
        if (!byMonitor || strip.monitor === monitor)
            return;
        byMonitor.delete(strip.monitor);
        strip.monitor = monitor;
        byMonitor.set(monitor, strip);
        this._glides.get(strip)?.timeline.stop();
        this._glides.delete(strip);
        this._boxes.get(strip)?.destroy();
        this._boxes.delete(strip);
    }

    // None once the engine has stopped: things that outlive it (the cards zooming at a screen lock) ask all the same
    strips() {
        return [...this._strips?.values() ?? []].flatMap(byMonitor => [...byMonitor.values()]);
    }

    connectorOf(monitor) {
        return this._connectors?.[monitor] ?? null;
    }

    indexOf(connector) {
        return connector ? this._connectors?.indexOf(connector) ?? -1 : -1;
    }

    // The monitor the keyboard is on: with several screens the one showing GNOME's workspace, else the focused
    // window's, or the pointer's
    focusMonitor() {
        if (this.screens?.separate) {
            const showing = this.screens.monitorShowing(global.workspace_manager.get_active_workspace());
            if (showing >= 0)
                return showing;
        }
        const focus = global.display.focus_window;
        if (focus && !focus.is_override_redirect())
            return this.stripOf(focus)?.monitor ?? focus.get_monitor();
        return global.display.get_current_monitor();
    }

    // The strip a screen shows
    activeStrip(monitor = this.focusMonitor()) {
        const workspace = this.screens?.shownOn(monitor) ?? global.workspace_manager.get_active_workspace();
        return this.strip(workspace, monitor);
    }

    area(monitor) {
        return Main.layoutManager.getWorkAreaForMonitor(monitor);
    }

    // The window a command acts on: the one a navigation is on, else the focused one
    current() {
        if (this.session)
            return this.session.strip.selected;
        return global.display.focus_window;
    }

    isTiled(window) {
        return !!this._info.get(window)?.strip;
    }

    // Whether a window belongs in a strip at all
    tileable(window) {
        if (!window || window.is_override_redirect() || window.get_window_type() !== Meta.WindowType.NORMAL)
            return false;
        if (window.get_transient_for() || window.is_on_all_workspaces() || this.floating.has(window) || isPreviewer(window))
            return false;
        return true;
    }

    // ---- Windows coming and going ----

    _created(window) {
        if (window.get_window_type() !== Meta.WindowType.NORMAL)
            return;
        const info = this._track(window);
        if (!this.tileable(window))
            return;
        info.fresh = true;
        // Every new tiled window gets the keyboard once it is drawn, whatever GNOME thought of its asking, unless
        // a rule opened it on another workspace without going there
        let drawn = false;
        window.get_compositor_private()?.connectObject('first-frame', () => {
            drawn = true;
            if (!info.fresh && !info.elsewhere && (this.isTiled(window) || this.floating.has(window)))
                activate(window);
            if (info.refullscreen)
                this._refullscreen(window, info);
        }, this);
        // The app chooses its width, and says its title, when it first draws: the strip takes it in then
        const settle = () => {
            if (!info.fresh || !this._info?.has(window))
                return;
            info.fresh = false;
            // A dialog says whose it is after it is made: it floats, as GNOME has it
            if (!this.tileable(window))
                return;
            // A window a saved set is opening goes to the set's workspace and its place there (workspaces/sets.js)
            const claim = this.tiling._ext.modules?.get('workspaces')?.claim?.(window);
            if (claim && this.placeClaimed(window, claim, {drawn})) {
                info.rule = {};
                return;
            }
            const entry = ruleFor(window);
            const rule = info.rule = entry?.rule ?? {};
            forget(entry, this.tiling.settings);
            if (rule.scratch_layer) {
                this.float(window, {glide: false});
                this._centreFloated(window, info, drawn);
                if (drawn)
                    activate(window);
                return;
            }
            // A new window goes where the person is looking, not where its app asked, unless a rule names a
            // workspace; and it takes the keyboard there only when the rule says so
            const active = global.workspace_manager.get_active_workspace();
            const target = this._ruleWorkspace(rule) ?? active;
            if (window.get_workspace() !== target)
                window.change_workspace(target);
            info.elsewhere = target !== active && !rule.focus;
            const monitor = this.focusMonitor();
            const width = preferredWidth(rule, Layout.available(this.area(monitor)));
            if (width)
                info.width = width;
            // Opening below (or above) the focused window, it is shown to its right first and pulled in a moment
            // later, so that it is seen arriving
            const strip = this.strip(target, monitor);
            const position = this.tiling.openPosition;
            const near = strip.selected;
            const stacked = near && (position === Open.DOWN || position === Open.UP);
            this.insert(window, strip, stacked ? {appear: true, position: Open.RIGHT} : {appear: true});
            if (stacked) {
                this._after(window, info, PULL_IN, () => {
                    if (this.stripOf(window) === strip && this.stripOf(near) === strip && !window.is_fullscreen())
                        this.insert(window, strip, {position, near});
                });
            }
            // A window that opens fullscreen (a game) has been put in the strip as a normal window: it leaves
            // fullscreen and takes it again from its place there, a moment after it is first drawn
            if (window.is_fullscreen()) {
                if (drawn)
                    this._refullscreen(window, info);
                else
                    info.refullscreen = true;
            }
            if (target !== active && rule.focus)
                target.activate_with_focus(window, global.get_current_time());
            else if (drawn && !info.elsewhere)
                activate(window);
        };
        if (window.get_frame_rect().width > 0)
            settle();
        else
            window.connectObject('size-changed', settle, this);
    }

    // A window of a saved set, new or already open: on the set's workspace, in the set's order among the set's
    // windows already in that workspace's strip, at its saved width as far as the screen allows. A new one gets the
    // keyboard once drawn, unless the person went to another workspace meanwhile. False, and nothing done, when the
    // set's workspace is gone (dynamic workspaces).
    placeClaimed(window, claim, {drawn = false, appear = true} = {}) {
        const manager = global.workspace_manager;
        const target = claim.workspace;
        if (![...Array(manager.n_workspaces).keys()].some(i => manager.get_workspace_by_index(i) === target))
            return false;
        const info = this._track(window);
        const active = manager.get_active_workspace();
        info.elsewhere = target !== active;
        const monitor = this.screens?.separate ? this.screens.placeOf(target, this.focusMonitor()) : this.focusMonitor();
        const strip = this.strip(target, monitor);
        info.width = Math.min(claim.tile.width, Layout.available(this.area(strip.monitor)));
        this.insert(window, strip, {appear, place: claim.slotIn(strip, window)});
        // A window that opens fullscreen takes it again from its place in the strip, as any new window does
        if (appear && window.is_fullscreen()) {
            if (drawn)
                this._refullscreen(window, info);
            else
                info.refullscreen = true;
        }
        if (appear && drawn && !info.elsewhere)
            activate(window);
        return true;
    }

    _track(window) {
        if (this._info.has(window))
            return this._info.get(window);
        const info = {
            width: 0, height: 0, lastX: null, everFocused: false, ...remembered.windows.get(window),
            strip: null, vis: null, target: null, hidden: false,
        };
        this._info.set(window, info);
        window.connectObject(
            'unmanaging', () => this._unmanaging(window),
            'size-changed', () => this._sizeChanged(window),
            'position-changed', () => this._positionChanged(window),
            'workspace-changed', () => this._workspaceOfWindowChanged(window),
            'notify::minimized', () => this._minimizedChanged(window),
            'notify::fullscreen', () => this._fullscreenChanged(window),
            'notify::maximized-horizontally', () => this._maximizedChanged(window),
            'notify::maximized-vertically', () => this._maximizedChanged(window),
            'notify::on-all-workspaces', () => this._stickyChanged(window),
            'notify::window-type', () => this._kindChanged(window),
            this);
        window.get_compositor_private()?.connectObject('notify::visible', actor => {
            // GNOME shows a window again on its own occasions: one off the screen stays hidden
            if (actor.visible && this._info?.get(window)?.hidden)
                actor.hide();
            this._boxFollows(actor);
        }, 'effects-completed', actor => {
            // While GNOME drew the window itself (busy: its own minimize and way back, an open, a change of size) the
            // strip moved it nowhere; once that is over it is drawn where the strip has it now
            const now = this._info?.get(window);
            if (!now?.strip || !now.vis || busy(actor))
                return;
            const frame = window.get_frame_rect();
            actor.translation_x = now.vis.x - frame.x;
            actor.translation_y = now.vis.y - frame.y;
            this._boxFollows(actor);
        }, this);
        return info;
    }

    // The workspace a window rule opens a window on: null for none, and for one that is not there, which is said
    _ruleWorkspace(rule) {
        if (!Number.isInteger(rule.spaceIndex))
            return null;
        const manager = global.workspace_manager;
        if (rule.spaceIndex >= 0 && rule.spaceIndex < manager.n_workspaces)
            return manager.get_workspace_by_index(rule.spaceIndex);
        Main.notify(_('A window rule names a workspace there is not'),
            _('The window opened on this workspace instead of workspace %d.').format(rule.spaceIndex + 1));
        return null;
    }

    // Runs `fn` once after `ms`, if the window is still the tiling's then
    _after(window, info, ms, fn) {
        info.timers ??= new Set();
        const id = GLib.timeout_add(GLib.PRIORITY_DEFAULT, ms, () => {
            info.timers.delete(id);
            if (this._info?.get(window) === info)
                fn();
            return GLib.SOURCE_REMOVE;
        });
        info.timers.add(id);
    }

    _refullscreen(window, info) {
        info.refullscreen = false;
        this._after(window, info, REFULLSCREEN, () => {
            if (window.is_fullscreen()) {
                window.unmake_fullscreen();
                window.make_fullscreen();
            }
        });
    }

    // A window a rule floats as it opens floats as in classic GNOME, which centres new windows (center-new-windows):
    // in the middle of its own monitor's work area, once it is first drawn, after Mutter's own placement. Mutter then
    // answers its first commits with the place it asked for before it was placed (the top left corner, seen with GTK 4
    // windows): a move away in the first half second is undone, once, unless the person has taken the window.
    _centreFloated(window, info, drawn) {
        const centre = () => {
            this._dropCentre(window, info);
            if (this._info?.get(window) !== info)
                return;
            const area = this.area(window.get_monitor());
            const frame = window.get_frame_rect();
            const x = area.x + Math.max(0, Math.round((area.width - frame.width) / 2));
            const y = area.y + Math.max(0, Math.round((area.height - frame.height) / 2));
            window.move_frame(true, x, y);
            const until = GLib.get_monotonic_time() + 500000;
            info.centreHold = window.connect('position-changed', () => {
                this._dropCentre(window, info);
                const now = window.get_frame_rect();
                if (GLib.get_monotonic_time() < until && (now.x !== x || now.y !== y))
                    window.move_frame(true, x, y);
            });
        };
        const actor = window.get_compositor_private();
        if (drawn || !actor) {
            centre();
            return;
        }
        info.centreActor = actor;
        info.centreFrame = actor.connect('first-frame', centre);
    }

    _dropCentre(window, info) {
        if (info.centreFrame)
            info.centreActor?.disconnect(info.centreFrame);
        if (info.centreHold)
            window.disconnect(info.centreHold);
        info.centreFrame = info.centreHold = 0;
        info.centreActor = null;
    }

    _untrack(window) {
        const info = this._info.get(window);
        if (info)
            this._dropCentre(window, info);
        if (info?.sizeCheck)
            GLib.source_remove(info.sizeCheck);
        for (const id of info?.timers ?? [])
            GLib.source_remove(id);
        window.disconnectObject(this);
        const actor = window.get_compositor_private();
        actor?.disconnectObject(this);
        this._info.delete(window);
    }

    // The window drawn where its frame is, and shown if GNOME shows it
    _unhide(window, info) {
        const actor = window.get_compositor_private();
        if (!actor)
            return;
        if (!busy(actor)) {
            actor.translation_x = 0;
            actor.translation_y = 0;
        }
        if (info.clipped) {
            actor.remove_clip();
            actor.remove_effect_by_name(CUT);
            info.clipped = false;
        }
        if (info.hidden) {
            info.hidden = false;
            if (window.showing_on_its_workspace() &&
                window.located_on_workspace(global.workspace_manager.get_active_workspace()))
                actor.show();
        }
    }

    // Puts a window back the way GNOME would have it
    _release(window, info) {
        this._unhide(window, info);
        if (!info.strip || window.is_fullscreen() || !window.get_compositor_private())
            return;
        // Nothing stays off its screen: a window the strip had pushed aside comes back inside it, as large as fits
        const area = this.area(info.strip.monitor);
        const frame = window.get_frame_rect();
        const width = Math.min(frame.width, area.width);
        const height = Math.min(frame.height, area.height);
        const x = Math.max(area.x, Math.min(frame.x, area.x + area.width - width));
        const y = Math.max(area.y, Math.min(frame.y, area.y + area.height - height));
        if (x !== frame.x || y !== frame.y || width !== frame.width || height !== frame.height)
            window.move_resize_frame(false, x, y, width, height);
    }

    _unmanaging(window) {
        const strip = this.stripOf(window);
        this.floating.delete(window);
        if (strip) {
            const order = strip.windows;
            const index = order.indexOf(window);
            const wasSelected = strip.selected === window;
            this.remove(window, {layout: false});
            if (wasSelected) {
                // The neighbour in strip order used most recently, by the last time each was used: GNOME's list of the
                // windows used last cannot be asked for while a window is going, which it still counts
                const before = order[index - 1], after = order[index + 1];
                let next = before ?? after ?? null;
                if (before && after && after.get_user_time() > before.get_user_time())
                    next = after;
                strip.selected = next;
                if (next && window.has_focus())
                    Main.activateWindow(next);
            }
            this.closeUp(strip);
        } else if (window.has_focus() && isPreviewer(window)) {
            // The keyboard goes back to the window it showed a file of, whatever GNOME would pick
            const owner = window.get_transient_for() ?? this.activeStrip().selected;
            if (owner)
                Main.activateWindow(owner);
        }
        this._untrack(window);
    }

    // `place` puts it at a given column, or into it at a given row, rather than beside a window. Stacked under or over
    // the selected window, as the open position may say, go only a new window and one put down after carrying it
    // (`position` given); any other (from another workspace or screen, back from floating or minimized, dropped
    // outside a zone) goes beside it.
    insert(window, strip, options = {}) {
        const {appear = false, place = null} = options;
        let position = options.position ?? this.tiling.openPosition;
        if (options.position === undefined && !appear && (position === Open.DOWN || position === Open.UP))
            position = Open.RIGHT;
        const info = this._track(window);
        const old = info.strip;
        if (old)
            this.remove(window, {layout: false});
        const near = options.near ?? strip.selected;
        if (window.is_above())
            window.unmake_above();
        if (config.maximizeFills && maximized(window)) {
            window.unmaximize();
            info.width = Layout.maxWidth(this.area(strip.monitor)) - 2 * (Layout.MARGIN - Layout.MIN_MARGIN);
        }
        if (window.get_workspace() !== strip.workspace && !window.is_on_all_workspaces())
            window.change_workspace(strip.workspace);
        const frame = window.get_frame_rect();
        if (!info.width)
            info.width = frame.width || Math.floor(Layout.available(this.area(strip.monitor)) / 2);
        info.height = 0;
        info.strip = strip;
        const at = near ? strip.find(near) : null;
        if (place && place.row >= 0 && strip.columns[place.column]) {
            const column = strip.columns[place.column];
            column.splice(Math.min(place.row, column.length), 0, window);
            for (const w of column)
                this._info.get(w).height = 0;
        } else if (place) {
            strip.columns.splice(Math.min(place.column, strip.columns.length), 0, [window]);
        } else if (position === Open.START) {
            strip.columns.unshift([window]);
        } else if (!at || position === Open.END) {
            strip.columns.push([window]);
        } else if (position === Open.DOWN || position === Open.UP) {
            const column = strip.columns[at[0]];
            column.splice(at[1] + (position === Open.DOWN ? 1 : 0), 0, window);
            for (const w of column)
                this._info.get(w).height = 0;
        } else {
            strip.columns.splice(at[0] + (position === Open.LEFT ? 0 : 1), 0, [window]);
        }
        strip.selected = window;
        // A brand-new window appears in its place; one coming from elsewhere glides from where it was
        if (appear)
            info.vis = null;
        else if (!info.vis)
            info.vis = this.visual(window);
        this.ensureVisible(strip, window, {appear: appear ? window : null});
        // The strip it came from closes up
        if (old && old !== strip)
            this.closeUp(old);
    }

    remove(window, {layout = true} = {}) {
        const info = this._info.get(window);
        const strip = info?.strip;
        if (!strip)
            return;
        const at = strip.find(window);
        if (at) {
            const column = strip.columns[at[0]];
            column.splice(at[1], 1);
            if (!column.length)
                strip.columns.splice(at[0], 1);
            else
                column.forEach(w => (this._info.get(w).height = 0));
        }
        if (strip.selected === window)
            strip.selected = strip.windows[0] ?? null;
        info.strip = null;
        info.target = null;
        const actor = window.get_compositor_private();
        this._glides.get(strip)?.windows.delete(window);
        // Out of the strip it is no longer cut at its screen's edge
        if (info.clipped) {
            actor?.remove_clip();
            actor?.remove_effect_by_name(CUT);
            info.clipped = false;
        }
        if (info.hidden) {
            info.hidden = false;
            if (window.showing_on_its_workspace())
                actor?.show();
        }
        if (layout)
            this.closeUp(strip);
    }

    // The strip after a window left it: closed up, and still showing the window it shows as focused
    closeUp(strip) {
        this.endAtEdge(strip);
        if (strip.selected)
            this.ensureVisible(strip, strip.selected);
        else
            this.layout(strip);
    }

    // A strip still wider than its screen leaves no room after its last column once one went or got narrower: its
    // right end comes to the screen's edge, at the margin
    endAtEdge(strip) {
        const geo = this.geometry(strip);
        if (geo.narrow || !geo.columns.length)
            return;
        const last = geo.columns.at(-1);
        const room = geo.area.width - Layout.MARGIN - (geo.scroll + last.x + last.width);
        if (room > 0)
            strip.scroll += room;
    }

    // ---- Floating ----

    // Takes a window out of the strip into the floating layer: above the others, on every workspace
    float(window, {glide = true} = {}) {
        const info = this._track(window);
        const strip = info.strip;
        const from = this.visual(window);
        if (strip)
            this.remove(window);
        this.floating.add(window);
        if (!window.is_above()) {
            window.make_above();
            info.madeAbove = true;
        }
        if (!window.is_on_all_workspaces()) {
            window.stick();
            info.madeSticky = true;
        }
        const actor = window.get_compositor_private();
        if (actor) {
            actor.translation_x = 0;
            actor.translation_y = 0;
        }
        // A window that was not in a strip (a dialog) stays where it is
        if (!glide || !strip)
            return;
        const monitor = window.get_monitor();
        let frame = info.floatFrame?.monitor === monitor ? {...info.floatFrame} : null;
        if (!frame) {
            const lower = 30;
            frame = {
                x: from.x, y: from.y + lower, width: from.width,
                height: Math.min(from.height - lower, Math.round(from.height * 0.9)),
            };
        }
        this.glideFloating(window, from, frame);
    }

    // Puts a floating window back into the strip of the workspace shown
    unfloat(window, {position, near} = {}) {
        const info = this._track(window);
        const frame = window.get_frame_rect();
        info.floatFrame = {
            x: frame.x, y: frame.y, width: frame.width, height: frame.height,
            monitor: window.get_monitor(),
        };
        this.floating.delete(window);
        this._unfloatFlags(window);
        // A dialog, or a window on every workspace, has no strip to go back to
        if (!this.tileable(window))
            return;
        const strip = this.activeStrip(window.get_monitor());
        info.width = frame.width;
        this.insert(window, strip, {position, near});
    }

    _unfloatFlags(window) {
        const info = this._info?.get(window) ?? remembered.windows.get(window) ?? {};
        if (info.madeAbove && window.is_above())
            window.unmake_above();
        if (info.madeSticky && window.is_on_all_workspaces())
            window.unstick();
        info.madeAbove = false;
        info.madeSticky = false;
    }

    // A floating window moved by the tiling, drawn gliding from `from` to `to`
    glideFloating(window, from, to) {
        window.move_resize_frame(false, to.x, to.y, to.width, to.height);
        const actor = window.get_compositor_private();
        const frame = window.get_frame_rect();
        if (!actor || busy(actor))
            return;
        actor.translation_x = from.x - frame.x;
        actor.translation_y = from.y - frame.y;
        actor.ease({
            translation_x: 0,
            translation_y: 0,
            duration: config.animation,
            mode: Clutter.AnimationMode.EASE_IN_OUT_QUAD,
            // Only a window still there: it may have closed during the glide
            onStopped: () => {
                if (this._info?.has(window))
                    window.activate(global.get_current_time());
            },
        });
    }

    // ---- Layout ----

    visual(window) {
        const frame = window.get_frame_rect();
        const actor = window.get_compositor_private();
        return {
            x: frame.x + Math.round(actor?.translation_x ?? 0),
            y: frame.y + Math.round(actor?.translation_y ?? 0),
            width: frame.width,
            height: frame.height,
        };
    }

    // Where every window of a strip goes, for its scroll as it is
    geometry(strip) {
        const area = this.area(strip.monitor);
        const use = Layout.usable(area);
        const monitor = Main.layoutManager.monitors[strip.monitor];
        const widths = strip.columns.map(column => {
            if (column.some(w => w.is_fullscreen()))
                return monitor.width;
            if (!config.maximizeFills && column.some(maximized))
                return area.width;
            const holder = column.includes(strip.selected) ? strip.selected : null;
            const width = holder ? this._info.get(holder).width
                : Math.max(...column.map(w => this._info.get(w).width));
            return Math.max(1, Math.min(width, Layout.maxWidth(area)));
        });
        const placed = Layout.columns(widths);
        // A strip narrower than the screen stays in the middle, unless a swipe, a drift or the centre key moved it
        // aside (`loose`): then it stays where it was put, until it is wide enough to cover the screen again
        const middle = Layout.centred(area, placed);
        if (middle === null || strip.scroll === middle)
            strip.loose = false;
        const centred = middle !== null && !strip.loose;
        if (centred)
            strip.scroll = middle;
        const scroll = strip.scroll;
        const rects = new Map();
        strip.columns.forEach((column, c) => {
            const x = area.x + scroll + placed.columns[c].x;
            const width = placed.columns[c].width;
            if (column.length === 1 && column[0].is_fullscreen()) {
                // Without the keyboard it slides down under the top bar, which comes back above it
                const y = column[0] === strip.selected ? monitor.y : use.y;
                rects.set(column[0], {x, y, width, height: monitor.height});
                return;
            }
            // A window maximized as GNOME has it covers the work area, the strip only scrolling it
            if (column.length === 1 && !config.maximizeFills && maximized(column[0])) {
                rects.set(column[0], {x, y: area.y, width, height: area.height});
                return;
            }
            const keep = column.indexOf(strip.selected);
            const hs = Layout.heights(use.height, column.map(w => this._info.get(w).height), keep);
            let y = use.y;
            column.forEach((w, r) => {
                rects.set(w, {x, y, width, height: hs[r]});
                y += hs[r] + Layout.GAP;
            });
        });
        return {area, rects, columns: placed.columns, width: placed.width, scroll, centred, narrow: middle !== null};
    }

    // Scrolls a strip so that a window is shown by the workspace's focus mode, then lays it out
    ensureVisible(strip, window, {animate = true, appear = null, from = null} = {}) {
        const at = strip.find(window);
        if (!at) {
            this.layout(strip, {animate, appear});
            return;
        }
        const geo = this.geometry(strip);
        const column = geo.columns[at[0]];
        const info = this._info.get(window);
        if (window.is_fullscreen()) {
            const monitor = Main.layoutManager.monitors[strip.monitor];
            strip.scroll = monitor.x - geo.area.x - column.x;
        } else if (!config.maximizeFills && maximized(window)) {
            strip.scroll = -column.x;
        } else if (!geo.centred) {
            const now = geo.scroll + column.x;
            const shown = now < geo.area.width && now + column.width > 0;
            const previous = from ?? -1;
            const neighbour = previous >= 0 && Math.abs(previous - at[0]) === 1;
            const start = neighbour || shown || !info.everFocused || info.lastX === null ? now : info.lastX;
            const x = Layout.placeFocused(geo.area, column, start, strip.mode, at[0], geo.columns.length);
            strip.scroll = Math.round(x - column.x);
        }
        this.layout(strip, {animate, appear});
    }

    layoutAll(animate = true) {
        for (const strip of this.strips())
            this.layout(strip, {animate});
    }

    // Puts every window of a strip in its place, gliding unless `animate` is false.
    //
    // A glide starts from where each window is drawn and at the speed it already has: a key pressed again while the
    // strip still moves, or fingers let go of it, carry on the motion instead of stopping it and starting it again,
    // and every glide comes to rest gently. `velocity` (pixels a millisecond, sideways) is the speed a gesture let
    // go at, `time` how long its momentum lasts; `mode` a curve of GNOME's for a glide that is not to follow the
    // motion (the overshoot at the end of a strip).
    layout(strip, {animate = true, appear = null, time = config.animation, mode = null, velocity = null} = {}) {
        if (!this._strips)
            return;
        const geo = this.geometry(strip);
        const showing = strip.workspace === global.workspace_manager.get_active_workspace();
        const duration = animate && showing ? adjustAnimationTime(time) : 0;
        const moving = [];
        for (const [window, rect] of geo.rects) {
            const info = this._info.get(window);
            info.target = rect;
            this._request(window, rect, strip);
            const start = window === appear || !info.vis ? rect : {...info.vis};
            const speed = velocity !== null ? {x: velocity, y: 0} : info.speed ?? {x: 0, y: 0};
            moving.push([window, start, rect, speed]);
        }
        const running = this._glides.get(strip);
        running?.timeline.stop();
        this._glides.delete(strip);
        // Nothing to glide: no frame is drawn for it
        const still = moving.every(([, from, to]) => from.x === to.x && from.y === to.y);
        if (!duration || still || this.dragging?.strip === strip) {
            for (const [window, , rect] of moving) {
                this._info.get(window).speed = null;
                this._show(window, rect, strip);
            }
            this._settled(strip);
            return;
        }
        // A glide that takes over from one still running starts from the moment that one last drew the windows, so
        // that the next frame already moves them on: started from now, it would draw them once more where they are
        const now = GLib.get_monotonic_time() / 1000;
        const start = mode === null && running?.drawn ? Math.max(now - 50, running.drawn) : now;
        const timeline = new Clutter.Timeline({actor: global.stage, duration: Math.max(1, Math.round(duration - (now - start)))});
        timeline.set_progress_mode(mode ?? Clutter.AnimationMode.LINEAR);
        const glide = {
            timeline,
            windows: new Map(moving.map(([w, from, to, speed]) => [w, {
                from, to, vx: carried(speed.x, to.x - from.x, duration), vy: carried(speed.y, to.y - from.y, duration),
            }])),
        };
        // How far along the glide is: by the clock for one that follows the motion, by GNOME's curve for another
        const progress = () => mode === null
            ? Math.min(1, (GLib.get_monotonic_time() / 1000 - start) / duration) : timeline.get_progress();
        const step = at => {
            glide.drawn = GLib.get_monotonic_time() / 1000;
            for (const [window, path] of glide.windows) {
                const info = this._info.get(window);
                const [x, vx] = mode === null ? hermite(path.from.x, path.to.x, path.vx, duration, at)
                    : [path.from.x + (path.to.x - path.from.x) * at, 0];
                const [y, vy] = mode === null ? hermite(path.from.y, path.to.y, path.vy, duration, at)
                    : [path.from.y + (path.to.y - path.from.y) * at, 0];
                if (info)
                    info.speed = at < 1 ? {x: vx, y: vy} : null;
                this._show(window, {x: Math.round(x), y: Math.round(y), width: path.to.width, height: path.to.height},
                    strip);
            }
            this.updateBox(strip);
        };
        timeline.connect('new-frame', () => step(progress()));
        timeline.connect('stopped', (t, finished) => {
            if (!finished)
                return;
            if (this._glides?.get(strip) === glide)
                this._glides.delete(strip);
            step(1);
            this._settled(strip);
        });
        this._glides.set(strip, glide);
        step(progress());
        timeline.start();
    }

    // Sets how far a strip has scrolled and draws it there at once: for gestures that follow a finger
    scrollTo(strip, scroll) {
        this._glides.get(strip)?.timeline.stop();
        this._glides.delete(strip);
        strip.scroll = Math.round(scroll);
        // A narrow strip moved by hand stays where it is put
        strip.loose = true;
        const geo = this.geometry(strip);
        for (const [window, rect] of geo.rects) {
            const info = this._info.get(window);
            info.target = rect;
            info.speed = null;
            this._show(window, rect, strip);
        }
        this.updateBox(strip);
        return geo;
    }

    // After a strip comes to rest: where each window was, for coming back to it, and the frames
    _settled(strip) {
        const geo = this.geometry(strip);
        for (const [window, rect] of geo.rects) {
            this._request(window, rect, strip);
            if (window === strip.selected)
                this._info.get(window).lastX = rect.x - geo.area.x;
        }
        this.updateBox(strip);
        // A fullscreen window scrolled away, or back: the top bar follows (topbar.js)
        if (strip.windows.some(w => w.is_fullscreen()))
            Main.layoutManager._updateVisibility();
        // The picture of a workspace another screen shows follows its strip
        if (strip.workspace !== global.workspace_manager.get_active_workspace())
            this.screens?.queueMirrors();
    }

    // Whether a fullscreen window really covers a monitor: GNOME counts one the strip has scrolled away, or slid down
    // under the top bar, too
    coversMonitor(index) {
        const monitor = Main.layoutManager.monitors[index];
        if (!monitor)
            return false;
        return global.display.list_all_windows().some(window => {
            if (!window.is_fullscreen() || window.minimized || window.get_monitor() !== index)
                return false;
            const info = this._info.get(window);
            if (!info?.strip)
                return true;
            return !info.hidden && info.vis && Math.abs(info.vis.x - monitor.x) < monitor.width / 2 &&
                info.vis.y <= monitor.y;
        });
    }

    // Asks GNOME for a window's frame as close to its place as it allows
    _request(window, rect, strip) {
        if (window.is_fullscreen() || this.dragging?.window === window || (!config.maximizeFills && maximized(window)))
            return;
        const monitor = Main.layoutManager.monitors[strip.monitor];
        const several = Main.layoutManager.monitors.length > 1;
        let x = rect.x;
        if (several) {
            // Kept on its own monitor, so that GNOME counts it as there
            x = Math.max(monitor.x, Math.min(x, monitor.x + monitor.width - Math.min(rect.width, monitor.width)));
        } else {
            x = Math.max(monitor.x + ONSCREEN - rect.width, Math.min(x, monitor.x + monitor.width - ONSCREEN));
        }
        const frame = window.get_frame_rect();
        const info = this._info.get(window);
        // A new size is an answer the app owes: when it was asked, for telling its answer from its own choice
        if (!info.asked || info.asked.width !== rect.width || info.asked.height !== rect.height)
            info.askedAt = GLib.get_monotonic_time() / 1000;
        info.asked = {x, y: rect.y, width: rect.width, height: rect.height};
        if (frame.x !== x || frame.y !== rect.y || frame.width !== rect.width || frame.height !== rect.height)
            window.move_resize_frame(false, x, rect.y, rect.width, rect.height);
    }

    // Draws a window at `vis`, hiding it when none of it is on its monitor
    _show(window, vis, strip) {
        const info = this._info.get(window);
        const actor = window.get_compositor_private();
        if (!info || !actor)
            return;
        info.vis = vis;
        if (!busy(actor)) {
            const frame = window.get_frame_rect();
            actor.translation_x = vis.x - frame.x;
            actor.translation_y = vis.y - frame.y;
        }
        const monitor = Main.layoutManager.monitors[strip.monitor];
        const onScreen = vis.x < monitor.x + monitor.width && vis.x + vis.width > monitor.x;
        this._clip(actor, info, vis, monitor);
        if (onScreen === !info.hidden)
            return;
        info.hidden = !onScreen;
        if (info.hidden)
            actor.hide();
        else if (window.showing_on_its_workspace() &&
            window.located_on_workspace(global.workspace_manager.get_active_workspace()))
            actor.show();
    }

    // With several screens a window that reaches past its own is cut at its edge, so that none of it shows on the
    // next screen; never while GNOME takes a screenshot, which takes windows whole
    _clip(actor, info, vis, monitor) {
        // GNOME (or the motion's drop) draws the window meanwhile, away from where the strip has it: not cut while
        // it flies; the next _show after it cuts it again
        if (busy(actor)) {
            if (info.clipped) {
                actor.remove_clip();
                actor.remove_effect_by_name(CUT);
                info.clipped = false;
            }
            return;
        }
        // A fullscreen window slid down reaches past the bottom
        const past = vis.x < monitor.x || vis.x + vis.width > monitor.x + monitor.width ||
            vis.y + vis.height > monitor.y + monitor.height;
        const wanted = past && !this.unclipped && Main.layoutManager.monitors.length > 1;
        if (wanted) {
            const left = actor.x + actor.translation_x;
            const top = actor.y + actor.translation_y;
            actor.set_clip(monitor.x - left, monitor.y - top, monitor.width, monitor.height);
            // Mutter leaves out what it thinks a window covers when it draws the desktop, without looking at the
            // cut: an effect on the window makes it look again
            if (!info.clipped)
                actor.add_effect_with_name(CUT, new Uncovered());
            info.clipped = true;
        } else if (info.clipped) {
            actor.remove_clip();
            actor.remove_effect_by_name(CUT);
            info.clipped = false;
        }
    }

    // GNOME's screenshot tool opened or closed: the cut edges go and come back
    setUnclipped(unclipped) {
        this.unclipped = unclipped;
        for (const strip of this.strips()) {
            for (const window of strip.windows) {
                const info = this._info.get(window);
                const actor = window.get_compositor_private();
                if (info?.vis && actor)
                    this._clip(actor, info, info.vis, Main.layoutManager.monitors[strip.monitor]);
            }
        }
    }

    _sizeChanged(window) {
        const info = this._info?.get(window);
        // A maximized frame is on its way to filling the width instead, and a fullscreen one is the screen's
        if (!info?.strip || window.is_fullscreen() || info.unmaximizing ||
            window.maximized_horizontally || window.maximized_vertically)
            return;
        const frame = window.get_frame_rect();
        const grab = this.dragging?.window === window || this._resizing === window;
        // The person dragging an edge chose another size: the strip takes it
        if (grab) {
            info.width = frame.width;
            info.height = frame.height;
            this.layout(info.strip, {animate: false});
            return;
        }
        if (info.vis)
            this._show(window, info.vis, info.strip);
        // So did the app, if its width is not the one last asked for: an app answers several requests in turn, and
        // only what it says once it has had the time to answer the last one is its choice, which the strip takes then
        if (info.asked && frame.width !== info.asked.width && !info.sizeCheck) {
            const take = () => {
                const now = window.get_frame_rect();
                if (this._info?.get(window) === info && info.strip && info.asked && !window.is_fullscreen() &&
                    !maximized(window) && now.width !== info.asked.width) {
                    info.width = now.width;
                    this.layout(info.strip);
                }
            };
            const wait = Math.ceil((info.askedAt ?? 0) + ANSWER - GLib.get_monotonic_time() / 1000);
            if (wait <= 0) {
                take();
                return;
            }
            info.sizeCheck = GLib.timeout_add(GLib.PRIORITY_DEFAULT, wait, () => {
                info.sizeCheck = 0;
                take();
                return GLib.SOURCE_REMOVE;
            });
        }
    }

    _positionChanged(window) {
        const info = this._info?.get(window);
        if (!info?.strip || !info.vis || this.dragging?.window === window || this._resizing === window)
            return;
        const frame = window.get_frame_rect();
        // Something other than the tiling moved a tiled window: it goes back, once per change it makes
        if (info.asked && !window.is_fullscreen() && !this._glides.has(info.strip) &&
            (frame.x !== info.asked.x || frame.y !== info.asked.y) && info.corrected !== `${frame.x},${frame.y}`) {
            info.corrected = `${frame.x},${frame.y}`;
            window.move_frame(false, info.asked.x, info.asked.y);
            return;
        }
        this._show(window, info.vis, info.strip);
        if (info.strip.selected === window)
            this.updateBox(info.strip);
    }

    // ---- Focus ----

    _focusChanged() {
        const window = global.display.focus_window;
        if (this.session || !window) {
            this._syncBoxes();
            return;
        }
        const info = this._info.get(window);
        if (info?.strip) {
            const strip = info.strip;
            const previous = strip.selected ? strip.find(strip.selected)?.[0] ?? -1 : -1;
            strip.selected = window;
            this._fullscreenAbove(strip, window);
            this.ensureVisible(strip, window, {from: previous});
            info.everFocused = true;
            this.restack(strip, window);
        }
        this._syncBoxes();
    }

    // A fullscreen window stays above everything only while it has the keyboard
    _fullscreenAbove(strip, focused) {
        for (const window of strip.windows) {
            const info = this._info.get(window);
            if (!window.is_fullscreen() || info.wasAbove)
                continue;
            if (window === focused && !window.is_above())
                window.make_above();
            else if (window !== focused && window.is_above())
                window.unmake_above();
        }
    }

    // A tiled window being resized by its edge, or dragged by its title bar
    _grabBegan(window, op) {
        // The person moves or sizes a window that has just been centred: where they take it is theirs
        const held = this._info.get(window);
        if (held?.centreHold)
            this._dropCentre(window, held);
        if (!this.isTiled(window))
            return;
        if (isResize(op))
            this._resizing = window;
    }

    _grabEnded(window) {
        if (this._resizing !== window)
            return;
        this._resizing = null;
        const info = this._info.get(window);
        if (info?.strip) {
            const frame = window.get_frame_rect();
            info.width = frame.width;
            info.height = frame.height;
            this.ensureVisible(info.strip, window);
        }
    }

    // The focused window on top, and its neighbours in the strip right under it, so that when it closes GNOME
    // gives the keyboard to one of them
    restack(strip, window) {
        const order = strip.windows;
        const index = order.indexOf(window);
        // The one used last raised last, by the last time each was used: this runs while GNOME hands the keyboard on
        // from a window that is going, when its list of the windows used last cannot be asked for
        const neighbours = [order[index - 1], order[index + 1]].filter(Boolean);
        neighbours.sort((a, b) => a.get_user_time() - b.get_user_time());
        for (const w of neighbours)
            w.raise();
        window.raise();
    }

    // Gives a strip's selected window the keyboard
    focus(strip) {
        const window = strip.selected;
        if (!window)
            return;
        const focus = global.display.focus_window;
        if (focus === window || (focus && focus.get_transient_for() === window))
            this._focusChanged();
        else
            Main.activateWindow(window);
    }

    _workspaceChanged() {
        const workspace = global.workspace_manager.get_active_workspace();
        // With several screens the workspace is shown where it belongs first (screens.js)
        const monitor = this.screens?.switched(workspace) ?? -1;
        for (const strip of this.strips()) {
            if (strip.workspace === workspace)
                this.layout(strip, {animate: false});
        }
        // The strip's own focused window, rather than whatever GNOME found on that workspace
        const strip = monitor >= 0 ? this.strip(workspace, monitor) : this.activeStrip();
        if (strip.selected && !this.session && !this.dragging)
            this.focus(strip);
        this._syncBoxes();
    }

    _workspaceOfWindowChanged(window) {
        const info = this._info?.get(window);
        if (!info?.strip || window.is_on_all_workspaces())
            return;
        const workspace = window.get_workspace();
        if (!workspace || workspace === info.strip.workspace)
            return;
        // Moved to another workspace by GNOME (Super+Shift+number): it joins that workspace's strip
        const target = this.strip(workspace, info.strip.monitor);
        this.remove(window);
        this.insert(window, target, {near: target.selected});
    }

    // A window that became a dialog leaves the strip
    _kindChanged(window) {
        if (this.isTiled(window) && !this.tileable(window))
            this.remove(window);
    }

    // A window put on every workspace belongs to no strip and is left where it is; put back on one, it joins the
    // strip shown
    _stickyChanged(window) {
        const info = this._info?.get(window);
        if (!info || this.floating.has(window))
            return;
        if (info.strip && window.is_on_all_workspaces()) {
            info.stuck = true;
            this.remove(window);
        } else if (!window.is_on_all_workspaces() && this._unpinned?.delete(window)) {
            // Out of the classic layout (enable): to the workspace of its own screen, not GNOME's
            info.stuck = false;
            const monitor = window.get_monitor();
            const workspace = this.screens?.shownOn(monitor) ?? global.workspace_manager.get_active_workspace();
            if (this.tileable(window) && !window.minimized)
                this.insert(window, this.strip(workspace, monitor));
            else if (window.get_workspace() !== workspace)
                window.change_workspace(workspace);
        } else if (info.stuck && !window.is_on_all_workspaces()) {
            info.stuck = false;
            if (this.tileable(window) && !window.minimized)
                this.insert(window, this.activeStrip());
        }
    }

    _minimizedChanged(window) {
        const info = this._info?.get(window);
        if (!info)
            return;
        if (window.minimized && info.strip) {
            info.fromStrip = true;
            this.remove(window);
        } else if (!window.minimized && info.fromStrip) {
            info.fromStrip = false;
            if (!this.tileable(window))
                return;
            // Back beside the focused window of the workspace shown where the keyboard is, not of the screen it was
            // minimized on; that window stays the one shown as focused until the keyboard moves
            const strip = this.activeStrip();
            const focused = strip.selected;
            this.insert(window, strip);
            if (focused && !window.has_focus() && global.display.focus_window === focused) {
                strip.selected = focused;
                this.ensureVisible(strip, focused);
            }
        }
    }

    _fullscreenChanged(window) {
        const info = this._info?.get(window);
        const strip = info?.strip;
        if (!strip)
            return;
        if (window.is_fullscreen()) {
            info.fullscreenWidth = info.width;
            info.wasAbove = window.is_above();
            // A stacked window gets a column of its own to cover the screen from
            const at = strip.find(window);
            if (at && strip.columns[at[0]].length > 1) {
                strip.columns[at[0]].splice(at[1], 1);
                strip.columns.splice(at[0] + 1, 0, [window]);
            }
            if (!info.wasAbove)
                window.make_above();
        } else {
            if (info.fullscreenWidth)
                info.width = info.fullscreenWidth;
            if (!info.wasAbove && window.is_above())
                window.unmake_above();
        }
        this.ensureVisible(strip, window);
        this._syncBoxes();
    }

    _maximizedChanged(window) {
        const info = this._info?.get(window);
        // Maximized as GNOME has it, the window covers the screen in its place in the strip until it is restored
        if (info?.strip && !config.maximizeFills) {
            this.ensureVisible(info.strip, window);
            return;
        }
        if (!info?.strip || !maximized(window) || info.unmaximizing)
            return;
        // Maximizing a tiled window fills the width instead, and maximizing again goes back. Done once GNOME has
        // finished maximizing it, before anything is drawn.
        info.unmaximizing = true;
        const width = info.width;
        global.compositor.get_laters().add(Meta.LaterType.BEFORE_REDRAW, () => {
            info.unmaximizing = false;
            if (this._info?.get(window) === info && info.strip) {
                window.unmaximize();
                info.width = width;
                this.tiling.actions?.fillWidth(window);
            }
            return false;
        });
    }

    // The strip comes to the focused window gliding, at once, or at once and fading in, as the settings say
    _overviewHidden() {
        let strip = this.activeStrip(global.display.get_current_monitor());
        const focus = global.display.focus_window;
        const animate = config.overviewExit === 1;
        if (focus && this.stripOf(focus)) {
            strip = this.stripOf(focus);
            this.ensureVisible(strip, focus, {animate});
        } else if (strip.selected) {
            this.layout(strip, {animate});
        }
        if (config.overviewExit !== 2)
            return;
        for (const window of strip.windows) {
            const actor = window.get_compositor_private();
            actor?.set_opacity(0);
            actor?.ease({opacity: 255, duration: config.animation, mode: Clutter.AnimationMode.EASE_OUT_QUAD});
        }
    }

    // ---- The focus box ----

    _box(strip) {
        let box = this._boxes.get(strip);
        if (!box) {
            box = new St.Widget({style_class: 'tile-preview amethystora-tiling-focus', reactive: false});
            global.window_group.add_child(box);
            this._boxes.set(strip, box);
        }
        return box;
    }

    updateBox(strip) {
        if (!this._boxes)
            return;
        const window = strip.selected;
        const info = window ? this._info.get(window) : null;
        const actor = window?.get_compositor_private();
        const showing = strip.workspace === global.workspace_manager.get_active_workspace();
        // What else follows the windows as they glide (the dimming of a navigation)
        this.session?.follow?.(strip);
        // The box is filled, and the window covers all but its edge: not before the window is drawn, nor while it
        // opens or comes back small, when it would stand there as a coloured sheet (_boxFollows brings it back)
        if (!info?.vis || !showing || window.is_fullscreen() || this.boxHidden > 0 || info.hidden ||
            this.dragging?.window === window || config.border <= 0 || !actor?.visible ||
            Main.wm._mapping?.has(actor) || Main.wm._unminimizing?.has(actor)) {
            this._boxes.get(strip)?.hide();
            return;
        }
        const box = this._box(strip);
        this._shapeBox(box, window, info.vis);
        const focus = global.display.focus_window;
        const active = this.session ? this.session.strip === strip : focus === window;
        box.opacity = active ? 255 : INACTIVE;
        box.show();
        // Only when out of place: Clutter takes the box out and puts it back otherwise too, a relayout and a redraw of
        // the window group on every frame of a glide
        if (box.get_parent() === actor.get_parent() && box.get_next_sibling() !== actor)
            global.window_group.set_child_below_sibling(box, actor);
    }

    // A window was shown or began or ended its opening: the box of its strip follows, when it is that strip's focused one
    _boxFollows(actor) {
        const window = actor.meta_window;
        const info = this._info?.get(window);
        if (info?.strip?.selected === window)
            this.updateBox(info.strip);
    }

    // Where the frame goes round a window drawn at `vis`: it stands out on no side where the window reaches the
    // screen's edge, maximized that way
    _shapeBox(box, window, vis) {
        const outX = window.maximized_horizontally ? 0 : config.border;
        const outY = window.maximized_vertically ? 0 : config.border;
        box.set_position(vis.x - outX, vis.y - outY);
        // A window that cannot be resized (a program's own question) stays its own size in the place the strip gave it
        const size = window.resizeable ? vis : window.get_frame_rect();
        box.set_size(size.width + 2 * outX, size.height + 2 * outY);
        const style = `border-radius: ${config.radiusTop}px ${config.radiusTop}px ${config.radiusBottom}px ${config.radiusBottom}px;`;
        if (box.style !== style)
            box.style = style;
    }

    // A frame of its own for a strip drawn elsewhere (the picture of another screen's workspace), as it is when the
    // keyboard is not there; null when the strip shows none
    boxFor(strip) {
        const window = strip.selected;
        const info = window ? this._info.get(window) : null;
        if (!info?.vis || info.hidden || window.is_fullscreen() || config.border <= 0)
            return null;
        const box = new St.Widget({style_class: 'tile-preview amethystora-tiling-focus', reactive: false});
        this._shapeBox(box, window, info.vis);
        box.opacity = INACTIVE;
        return box;
    }

    _syncBoxes() {
        for (const strip of this.strips())
            this.updateBox(strip);
    }

    _restackBoxes() {
        for (const strip of this.strips()) {
            const actor = strip.selected?.get_compositor_private();
            const box = this._boxes.get(strip);
            if (box && actor && box.get_parent() === actor.get_parent() && box.get_next_sibling() !== actor)
                global.window_group.set_child_below_sibling(box, actor);
        }
    }

    // ---- Monitors and workspaces ----

    // The strip of a workspace on a screen as it was when a set of screens was last left (desks/layouts.js): each of
    // `windows` ({window, width, column, row}, in their order) at its place, from wherever it is now, and the strip's other
    // windows after them. Says how many moved.
    arrange(workspace, monitor, windows) {
        const strip = this.strip(workspace, monitor);
        let moved = 0;
        if (strip.monitor !== monitor) {
            this.moveStrip(strip, monitor);
            moved = strip.windows.length;
        }
        this.screens?.setHome(workspace, monitor);
        const selected = strip.selected;
        for (const {window, width, column, row} of windows) {
            if (!this.tileable(window) || window.minimized || this.floating.has(window))
                continue;
            const info = this._track(window);
            const wanted = Math.min(width, Layout.available(this.area(monitor)));
            const at = strip.find(window);
            if (at && at[0] === column && at[1] === row && info.width === wanted)
                continue;
            info.width = wanted;
            this.insert(window, strip, {place: row > 0 ? {column, row} : {column}});
            moved++;
        }
        if (selected && strip.find(selected))
            strip.selected = selected;
        if (strip.selected)
            this.ensureVisible(strip, strip.selected);
        else
            this.layout(strip);
        return moved;
    }

    // The screen shows this workspace, when each screen shows its own
    showOn(workspace, monitor) {
        if (this.screens?.separate && this.screens.monitorShowing(workspace) !== monitor)
            this.screens.show(workspace, monitor);
    }

    _monitorsChanged() {
        // What the strips still hold of the screens as they were (desks/layouts.js)
        this.tiling._ext.modules?.get('desks')?.layouts?.leaving();
        // A navigation is on a strip that is about to be merged or renumbered: it ends first, and quietly
        const released = this.tiling.sessions?.end({quiet: true}) ?? [];
        const count = Main.layoutManager.monitors.length;
        const primary = Main.layoutManager.primaryIndex;
        const before = this._connectors;
        const after = this._connectors = connectors();
        for (const [workspace, byMonitor] of this._strips) {
            const strips = [...byMonitor.values()];
            byMonitor.clear();
            const gone = [];
            for (const strip of strips) {
                // The strip stays with its screen, whatever number the screen has now
                const connector = before[strip.monitor];
                const index = connector ? after.indexOf(connector) : strip.monitor;
                if (index < 0 || index >= count) {
                    gone.push(strip);
                    continue;
                }
                strip.monitor = index;
                byMonitor.set(index, strip);
            }
            for (const strip of gone) {
                // A monitor that went away: with screens still showing a workspace each, its workspace's strip goes to
                // the primary screen whole; else its windows join the primary screen's strip of the same workspace
                if (this.screens?.separate && !byMonitor.size) {
                    this._glides.get(strip)?.timeline.stop();
                    this._glides.delete(strip);
                    strip.monitor = primary;
                    byMonitor.set(primary, strip);
                    continue;
                }
                const target = this.strip(workspace, primary);
                for (const column of strip.columns) {
                    for (const window of column)
                        this._info.get(window).strip = target;
                    target.columns.push(column);
                }
                target.selected ??= strip.selected;
                this._glides.get(strip)?.timeline.stop();
                this._glides.delete(strip);
                this._boxes.get(strip)?.destroy();
                this._boxes.delete(strip);
            }
        }
        // The windows the navigation was carrying are in no strip: the primary monitor's takes them
        for (const window of released) {
            if (this._info.has(window) && this.tileable(window))
                this.insert(window, this.activeStrip(primary));
        }
        this.tiling.monitorsChanged?.();
        // Every screen shows a workspace of its own again, the one it showed before when it did
        this.screens?.assign(this.tiling.workspaceHistory?.() ?? []);
        this.layoutAll(false);
    }

    _prune() {
        const workspaces = new Set(Array.from({length: global.workspace_manager.n_workspaces},
            (unused, i) => global.workspace_manager.get_workspace_by_index(i)));
        for (const [workspace, byMonitor] of [...this._strips]) {
            if (workspaces.has(workspace))
                continue;
            for (const strip of byMonitor.values()) {
                this._boxes.get(strip)?.destroy();
                this._boxes.delete(strip);
            }
            this._strips.delete(workspace);
        }
    }

    // ---- Starting and stopping ----

    _remember() {
        if (!this._strips)
            return;
        remembered.strips = this.strips().map(strip => ({
            workspace: strip.workspace, monitor: strip.monitor, connector: this._connectors?.[strip.monitor],
            columns: strip.columns.map(c => [...c]),
            scroll: strip.scroll, loose: strip.loose, mode: strip.mode, selected: strip.selected,
        }));
        remembered.windows = new WeakMap();
        for (const [window, info] of this._info) {
            const {width, height, lastX, everFocused, floatFrame, madeAbove, madeSticky, fromStrip, stuck} = info;
            remembered.windows.set(window, {
                width, height, lastX, everFocused, floatFrame, madeAbove, madeSticky,
                fromStrip, stuck,
            });
        }
        remembered.floating = new Set(this.floating);
    }

    // The windows already open: as they were when the tiling stopped in this session, else by where they are
    _restore() {
        const alive = new Set(global.display.list_all_windows());
        for (const window of remembered.floating) {
            if (alive.has(window) && !window.minimized)
                this.float(window, {glide: false});
        }
        const placed = new Set();
        const workspaces = new Set(Array.from({length: global.workspace_manager.n_workspaces},
            (unused, i) => global.workspace_manager.get_workspace_by_index(i)));
        for (const saved of remembered.strips) {
            // The screen it was on, by its connector, whatever number it has now
            const monitor = saved.connector ? this._connectors.indexOf(saved.connector) : saved.monitor;
            if (!workspaces.has(saved.workspace) || monitor < 0 || monitor >= Main.layoutManager.monitors.length)
                continue;
            const strip = this.strip(saved.workspace, monitor);
            Object.assign(strip, {scroll: saved.scroll, loose: saved.loose, mode: saved.mode});
            for (const column of saved.columns) {
                const kept = column.filter(w => alive.has(w) && this.tileable(w) && !w.minimized &&
                    w.located_on_workspace(saved.workspace));
                if (!kept.length)
                    continue;
                strip.columns.push(kept);
                for (const window of kept) {
                    this._track(window).strip = strip;
                    placed.add(window);
                }
            }
            strip.selected = alive.has(saved.selected) && placed.has(saved.selected) ? saved.selected
                : strip.windows[0] ?? null;
        }
        // Windows the tiling has not seen: from the left of the screen, the stacking breaking ties. Those at its left
        // edge or past it (a strip pushed them there) come first, in their stacking.
        const stacked = global.display.sort_windows_by_stacking(global.display.list_all_windows());
        const others = stacked.filter(w => !placed.has(w) && w.get_window_type() === Meta.WindowType.NORMAL);
        for (const window of others) {
            this._track(window);
            if (window.is_above() && this.tileable(window) && !window.minimized)
                this.floating.add(window);
        }
        const left = window => {
            const x = window.get_frame_rect().x;
            return x <= this.area(window.get_monitor()).x ? -Infinity : x;
        };
        const fresh = others.filter(w => this.tileable(w) && !w.minimized)
            .map(window => [window, left(window)])
            .sort(([, a], [, b]) => a === b ? 0 : a - b)
            .map(([window]) => window);
        for (const window of fresh) {
            const workspace = window.get_workspace();
            if (!workspace)
                continue;
            const strip = this.strip(workspace, window.get_monitor());
            const info = this._track(window);
            if (config.maximizeFills && maximized(window)) {
                window.unmaximize();
                info.width = Layout.maxWidth(this.area(strip.monitor)) - 2 * (Layout.MARGIN - Layout.MIN_MARGIN);
            }
            info.width ||= window.get_frame_rect().width;
            info.strip = strip;
            strip.columns.push([window]);
            strip.selected ??= window;
        }
        for (const window of others.filter(w => w.minimized && this.tileable(w)))
            this._info.get(window).fromStrip ??= true;
        const focus = global.display.focus_window;
        for (const strip of this.strips()) {
            if (focus && strip.find(focus))
                strip.selected = focus;
            if (strip.selected)
                this.ensureVisible(strip, strip.selected, {animate: false});
            else
                this.layout(strip, {animate: false});
        }
        this._syncBoxes();
    }
}

// A new window takes the keyboard, without closing the overview if it is open
function activate(window) {
    window.activate(global.get_current_time());
}

function maximized(window) {
    return window.maximized_horizontally || window.maximized_vertically;
}

// Where a glide is at `s` (0 to 1) of its `time`, and how fast it goes there, in pixels a millisecond: from `from`
// at speed `v` to `to` at rest, along the cubic that keeps both the place and the speed it started with. With no
// speed to start with it is an ease in and out.
function hermite(from, to, v, time, s) {
    const s2 = s * s, s3 = s2 * s;
    const place = (2 * s3 - 3 * s2 + 1) * from + (s3 - 2 * s2 + s) * time * v + (3 * s2 - 2 * s3) * to;
    const speed = ((6 * s2 - 6 * s) * (from - to)) / time + (3 * s2 - 4 * s + 1) * v;
    return [place, speed];
}

// The speed a glide starts with: the one a window already has, as long as it carries the window no further than
// its new place (more would overshoot it and come back). A window sent back the way it came turns at once rather
// than coasting on, and one that does not move that way has none.
function carried(v, distance, time) {
    if (!v || Math.abs(distance) < 1 || Math.sign(v) !== Math.sign(distance))
        return 0;
    const most = 2.5 * Math.abs(distance) / time;
    return Math.sign(v) * Math.min(Math.abs(v), most);
}

// A grab that changes a window's size, by the mouse or the keyboard (the unconstrained flag aside)
function isResize(op) {
    const plain = op & ~0x400;
    return (plain & 0xf000) !== 0 || plain === Meta.GrabOp.KEYBOARD_RESIZING_UNKNOWN;
}

// GNOME is animating the window itself (opening it, or a change of size): its translation is GNOME's meanwhile
function busy(actor) {
    return !!actor.__animationInfo || Main.wm._mapping?.has(actor) || Main.wm._resizing?.has(actor) ||
        Main.wm._minimizing?.has(actor) || Main.wm._unminimizing?.has(actor);
}
