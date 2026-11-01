// What each of the tiling's keys does. A key that moves around the strip starts a navigation session (session.js)
// and acts inside it; a key for the window itself acts on the window the session is on, or on the focused one, and
// starts nothing. The screen keys end a session first.

import Meta from 'gi://Meta';
import Shell from 'gi://Shell';

import * as Main from 'resource:///org/gnome/shell/ui/main.js';

import * as Layout from './layout.js';
import {config} from './config.js';
import {Open} from './engine.js';

// The columns the numbered keys go to
const ORDINALS = ['second', 'third', 'fourth', 'fifth', 'sixth', 'seventh', 'eighth', 'ninth', 'tenth', 'eleventh'];

// Keys that start or act in a navigation session, and whether they show the map of the strip
const SESSION = {
    'switch-left': true, 'switch-right': true, 'switch-up': true, 'switch-down': true,
    'switch-first': true, 'switch-last': true, 'move-left': true, 'move-right': true,
    'move-up': true, 'move-down': true, 'drift-left': false, 'drift-right': false, 'take-window': false,
    'switch-up-workspace': false, 'switch-down-workspace': false, 'move-up-workspace': false,
    'move-down-workspace': false, 'move-previous-workspace': false, 'move-previous-workspace-backward': false,
    'switch-next': true, 'switch-previous': true, 'switch-next-loop': true, 'switch-previous-loop': true,
    'switch-left-loop': true, 'switch-right-loop': true, 'switch-up-loop': true, 'switch-down-loop': true,
    'switch-global-left': true, 'switch-global-right': true, 'switch-global-up': true, 'switch-global-down': true,
    'switch-up-or-else-workspace': true, 'switch-down-or-else-workspace': true,
    'previous-workspace': false, 'previous-workspace-backward': false,
    'switch-up-workspace-from-all-monitors': false, 'switch-down-workspace-from-all-monitors': false,
    'live-alt-tab': false, 'live-alt-tab-backward': false, 'live-alt-tab-scratch': false,
    'live-alt-tab-scratch-backward': false,
    ...Object.fromEntries(ORDINALS.map(n => [`switch-${n}`, true])),
};

// Where a key that goes on past the strip goes once there is no window that way
const BEYOND = {
    'switch-global-left': 'monitor-left', 'switch-global-right': 'monitor-right',
    'switch-global-up': 'monitor-above', 'switch-global-down': 'monitor-below',
    'switch-up-or-else-workspace': 'workspace-up', 'switch-down-or-else-workspace': 'workspace-down',
};

const DIRECTIONS = {
    left: Meta.DisplayDirection.LEFT, right: Meta.DisplayDirection.RIGHT,
    above: Meta.DisplayDirection.UP, below: Meta.DisplayDirection.DOWN,
};

export class Actions {
    constructor(tiling) {
        this.tiling = tiling;
        this.engine = tiling.engine;
    }

    // name -> {flags, run(context)}, for keys.js
    table() {
        const once = Meta.KeyBindingFlags.IGNORE_AUTOREPEAT;
        const table = new Map();
        const add = (name, run, flags = Meta.KeyBindingFlags.NONE) => table.set(name, {run, flags});
        for (const name of Object.keys(SESSION))
            add(name, context => this.tiling.sessions.begin(name, context));
        const perWindow = {
            'slurp-in': w => this.slurpIn(w),
            'barf-out': w => this.barfOut(w, false),
            'barf-out-active': w => this.barfOut(w, true),
            'cycle-width': w => this.cycleWidth(w, true),
            'cycle-width-backwards': w => this.cycleWidth(w, false),
            'cycle-height': w => this.cycleHeight(w, true),
            'cycle-height-backwards': w => this.cycleHeight(w, false),
            'resize-w-inc': w => this.stepWidth(w, true),
            'resize-w-dec': w => this.stepWidth(w, false),
            'resize-h-inc': w => this.stepHeight(w, true),
            'resize-h-dec': w => this.stepHeight(w, false),
            'toggle-maximize-width': w => this.fillWidth(w),
            'center-horizontally': w => this.centre(w),
            'new-window': w => this.newWindow(w),
            'toggle-scratch': w => this.toggleFloating(w),
            'center-vertically': w => this.centreVertically(w),
            'center': w => this.centreBoth(w),
            'paper-toggle-fullscreen': w => this.toggleFullscreen(w),
            'close-window': w => w.delete(global.get_current_time()),
        };
        // Held down, these go on as the key repeats; the rest act once a press
        const repeats = new Set(['barf-out', 'barf-out-active', 'cycle-width', 'cycle-width-backwards', 'cycle-height',
            'cycle-height-backwards', 'resize-w-inc', 'resize-w-dec', 'resize-h-inc', 'resize-h-dec']);
        for (const [name, act] of Object.entries(perWindow)) {
            add(name, () => {
                const window = this.engine.current();
                if (window)
                    act(window);
            }, repeats.has(name) ? Meta.KeyBindingFlags.NONE : once);
        }
        add('toggle-scratch-layer', () => this.toggleFloatingLayer(), once);
        add('toggle-scratch-window', () => this.toggleLastFloating(), once);
        add('switch-focus-mode', () => this.cycleFocusMode(), once);
        add('switch-open-window-position', () => this.cycleOpenPosition(), once);
        for (const [name, value] of Object.entries(Open))
            add(`open-window-position-${name.toLowerCase()}`, () => this.setOpenPosition(value), once);
        add('toggle-top-and-position-bar', () => this.tiling.topBar?.toggle(), once);
        add('toggle-top-bar', () => this.tiling.topBar?.toggle('top'), once);
        add('toggle-position-bar', () => this.tiling.topBar?.toggle('position'), once);
        add('activate-window-under-cursor', () => this.activateUnderPointer(), once);
        for (const [side, direction] of Object.entries(DIRECTIONS)) {
            // Going to a screen, or taking a window there, goes on while held; swapping or moving a workspace acts
            // once, or a held key would flip it back and forth
            const monitorKey = (name, act, flags) => add(name, () => {
                this.tiling.sessions.end();
                act(direction);
            }, flags);
            monitorKey(`switch-monitor-${side}`, d => this.focusMonitor(d));
            monitorKey(`move-monitor-${side}`, d => this.moveToMonitor(d));
            monitorKey(`swap-monitor-${side}`, d => this.swapMonitor(d), once);
            monitorKey(`move-space-monitor-${side}`, d => this.moveSpaceToMonitor(d), once);
        }
        return table;
    }

    // Runs a key's action as if it was pressed
    run(name, context = {}) {
        this._table ??= this.table();
        this._table.get(name)?.run(context);
    }

    // The screen keys end a navigation session before they act
    endsSession(name) {
        return /^(switch|move|swap|move-space)-monitor-/.test(name);
    }

    showsMap(name) {
        return SESSION[name] === true;
    }

    isSessionKey(name) {
        return name in SESSION;
    }

    // ---- Moving around (inside a session) ----

    // The window a movement starts from and where it is
    _here(strip) {
        const window = strip.selected;
        const at = window ? strip.find(window) : null;
        return {window, at};
    }

    // Where a key that goes on past the strip goes ('monitor-left', 'workspace-up'...) once it has nowhere to go
    // in it, for the session to take it there
    beyond(name) {
        return BEYOND[name] ?? null;
    }

    // Moves a session's selection; returns whether anything changed
    move(name, strip) {
        const {window, at} = this._here(strip);
        if (!at)
            return false;
        const [c, r] = at;
        const columns = strip.columns;
        const wrap = (index, length) => (index + length) % length;
        const order = strip.windows;
        let target = null;
        const ordinal = ORDINALS.indexOf(name.replace(/^switch-/, ''));
        if (ordinal >= 0)
            name = 'switch-nth';
        switch (name) {
        case 'switch-left':
        case 'switch-right':
        case 'switch-global-left':
        case 'switch-global-right': {
            const column = columns[c + (/left/.test(name) ? -1 : 1)];
            target = column ? this._recent(column) : null;
            break;
        }
        case 'switch-left-loop':
        case 'switch-right-loop':
            target = this._recent(columns[wrap(c + (name === 'switch-left-loop' ? -1 : 1), columns.length)]);
            break;
        case 'switch-up':
        case 'switch-global-up':
        case 'switch-up-or-else-workspace':
            target = columns[c][r - 1] ?? null;
            break;
        case 'switch-down':
        case 'switch-global-down':
        case 'switch-down-or-else-workspace':
            target = columns[c][r + 1] ?? null;
            break;
        case 'switch-up-loop':
        case 'switch-down-loop':
            target = columns[c][wrap(r + (name === 'switch-up-loop' ? -1 : 1), columns[c].length)];
            break;
        case 'switch-next':
        case 'switch-previous':
            target = order[order.indexOf(window) + (name === 'switch-next' ? 1 : -1)] ?? null;
            break;
        case 'switch-next-loop':
        case 'switch-previous-loop':
            target = order[wrap(order.indexOf(window) + (name === 'switch-next-loop' ? 1 : -1), order.length)];
            break;
        case 'switch-nth':
            target = columns[ordinal + 1] ? this._recent(columns[ordinal + 1]) : null;
            break;
        case 'switch-first':
            target = columns[0][0];
            break;
        case 'switch-last':
            target = columns[columns.length - 1][0];
            break;
        case 'move-left':
        case 'move-right': {
            const other = c + (name === 'move-left' ? -1 : 1);
            if (other < 0 || other >= columns.length)
                return false;
            [columns[c], columns[other]] = [columns[other], columns[c]];
            this.engine.ensureVisible(strip, window, {from: c});
            return true;
        }
        case 'move-up':
        case 'move-down': {
            const other = r + (name === 'move-up' ? -1 : 1);
            const column = columns[c];
            if (other < 0 || other >= column.length)
                return false;
            [column[r], column[other]] = [column[other], column[r]];
            this.engine.ensureVisible(strip, window);
            return true;
        }
        }
        if (!target || target === window)
            return false;
        strip.selected = target;
        this.engine.ensureVisible(strip, target, {from: c});
        return true;
    }

    // The window of a column used last
    _recent(column) {
        let best = column[0], rank = Infinity;
        const mru = global.display.get_tab_list(Meta.TabList.NORMAL_ALL, null);
        for (const window of column) {
            const index = mru.indexOf(window);
            if (index >= 0 && index < rank) {
                best = window;
                rank = index;
            }
        }
        return best;
    }

    // ---- Columns ----

    // The side new windows open on decides where pulling in and pushing out go
    _towardsOpening() {
        return [Open.LEFT, Open.START].includes(this.tiling.openPosition) ? -1 : 1;
    }

    slurpIn(window) {
        const strip = this.engine.stripOf(window);
        const at = strip?.find(window);
        if (!at || strip.columns.length < 2)
            return;
        const neighbour = strip.columns[at[0] + this._towardsOpening()];
        if (!neighbour)
            return;
        const taken = neighbour.shift();
        if (taken.is_fullscreen())
            taken.unmake_fullscreen();
        if (!neighbour.length)
            strip.columns.splice(strip.columns.indexOf(neighbour), 1);
        const column = strip.columns[strip.find(window)[0]];
        column.push(taken);
        this._equalHeights(column);
        this.engine.info(taken).strip = strip;
        this.engine.ensureVisible(strip, window);
    }

    barfOut(window, itself) {
        const strip = this.engine.stripOf(window);
        const at = strip?.find(window);
        if (!at)
            return;
        const column = strip.columns[at[0]];
        if (column.length < 2)
            return;
        const leaving = itself ? window : column[column.length - 1];
        column.splice(column.indexOf(leaving), 1);
        const side = this._towardsOpening();
        strip.columns.splice(at[0] + (side > 0 ? 1 : 0), 0, [leaving]);
        this._equalHeights(column);
        this.engine.ensureVisible(strip, window);
    }

    _equalHeights(column) {
        for (const w of column)
            this.engine.info(w).height = 0;
    }

    // ---- Sizes ----

    _area(window) {
        const strip = this.engine.stripOf(window);
        return this.engine.area(strip ? strip.monitor : window.get_monitor());
    }

    _unmaximized(window) {
        if (window.maximized_horizontally || window.maximized_vertically)
            window.unmaximize();
    }

    // A floating window gets a new frame, kept on its screen with the least margin
    _floatingFrame(window, change) {
        const area = this._area(window);
        const frame = window.get_frame_rect();
        const next = {x: frame.x, y: frame.y, width: frame.width, height: frame.height, ...change};
        next.width = Math.min(next.width, area.width);
        next.height = Math.min(next.height, area.height);
        const right = area.x + area.width - Layout.MIN_MARGIN;
        if (next.x + next.width > right)
            next.x = Math.max(area.x, right - next.width);
        this.engine.glideFloating(window, this.engine.visual(window), next);
    }

    // A tiled window's width is the strip's, whatever the app has drawn yet; a floating one's its frame's
    _width(window) {
        const info = this.engine.info(window);
        if (info?.strip)
            return Math.min(info.width, Layout.maxWidth(this._area(window)));
        return window.get_frame_rect().width;
    }

    _setWidth(window, width) {
        const info = this.engine.info(window);
        const strip = info?.strip;
        if (!strip) {
            this._floatingFrame(window, {width});
            return;
        }
        info.width = Math.min(width, this._area(window).width);
        this.engine.endAtEdge(strip);
        this.engine.ensureVisible(strip, window);
    }

    cycleWidth(window, forward) {
        this._unmaximized(window);
        const now = this._width(window);
        this._setWidth(window, Layout.nextStep(now, Layout.available(this._area(window)), forward));
    }

    stepWidth(window, grow) {
        this._unmaximized(window);
        const now = this._width(window);
        this._setWidth(window, Layout.stepSize(now, Layout.available(this._area(window)), grow));
    }

    cycleHeight(window, forward) {
        this._unmaximized(window);
        const info = this.engine.info(window);
        const strip = info?.strip;
        const now = window.get_frame_rect().height;
        if (!strip) {
            this._floatingFrame(window, {height: Layout.nextStep(now, this._area(window).height, forward, Layout.HEIGHT_STEPS)});
            return;
        }
        const column = strip.columns[strip.find(window)[0]];
        if (column.length < 2)
            return;
        const space = Layout.usable(this._area(window)).height - Layout.GAP * (column.length - 1);
        this._equalHeights(column);
        info.height = Layout.nextStep(now, space, forward, Layout.HEIGHT_STEPS);
        this.engine.ensureVisible(strip, window);
    }

    stepHeight(window, grow) {
        this._unmaximized(window);
        const info = this.engine.info(window);
        const strip = info?.strip;
        const area = this._area(window);
        const space = Layout.usable(area).height - 2 * Layout.MARGIN - Layout.GAP;
        const height = Layout.stepSize(window.get_frame_rect().height, space, grow);
        if (!strip) {
            this._floatingFrame(window, {height});
            return;
        }
        info.height = height;
        this.engine.ensureVisible(strip, window);
    }

    // As wide as the screen allows, and back to the width it had
    fillWidth(window) {
        this._unmaximized(window);
        const area = this._area(window);
        const full = Math.round((area.width - 2 * Layout.MARGIN) * config.fillShare);
        const info = this.engine.info(window) ?? {};
        const frame = window.get_frame_rect();
        const wide = Math.abs(this._width(window) - full) <= Layout.SLACK;
        if (wide && info.beforeFill) {
            const before = info.beforeFill;
            info.beforeFill = null;
            if (info.strip)
                this._setWidth(window, before.width);
            else
                this._floatingFrame(window, {x: before.x, width: before.width});
            return;
        }
        info.beforeFill = {x: frame.x, width: this._width(window)};
        if (info.strip)
            this._setWidth(window, full);
        else
            this._floatingFrame(window, {x: area.x + Layout.MARGIN, width: full});
    }

    centre(window) {
        const info = this.engine.info(window);
        const strip = info?.strip;
        const area = this._area(window);
        if (!strip) {
            const frame = window.get_frame_rect();
            const top = Layout.usable(area).y;
            this._floatingFrame(window, {
                x: area.x + Math.round((area.width - frame.width) / 2),
                y: Math.max(top, frame.y),
            });
            return;
        }
        const geo = this.engine.geometry(strip);
        const column = geo.columns[strip.find(window)[0]];
        // A strip narrower than the screen leaves the middle for it, and stays there (engine.js, geometry)
        strip.scroll = Math.round((area.width - column.width) / 2) - column.x;
        strip.loose = true;
        this.engine.layout(strip);
    }

    // A floating window halfway down its screen's usable height; a tiled one already is
    centreVertically(window) {
        if (this.engine.isTiled(window))
            return;
        const area = Layout.usable(this._area(window));
        const frame = window.get_frame_rect();
        this._floatingFrame(window, {y: area.y + Math.max(0, Math.round((area.height - frame.height) / 2))});
    }

    centreBoth(window) {
        this.centre(window);
        this.centreVertically(window);
    }

    toggleFullscreen(window) {
        if (window.is_fullscreen())
            window.unmake_fullscreen();
        else
            window.make_fullscreen();
    }

    // On the window's own workspace, which is not always the one GNOME counts as current
    newWindow(window) {
        const app = Shell.WindowTracker.get_default().get_window_app(window);
        const workspace = window.is_on_all_workspaces() ? null : window.get_workspace();
        if (app?.can_open_new_window())
            app.open_new_window(workspace?.index() ?? -1);
    }

    // ---- The floating layer ----

    // Any window can float, a dialog too; one that does not tile just leaves the layer again
    toggleFloating(window) {
        if (this.engine.floating.has(window))
            this.engine.unfloat(window);
        else
            this.engine.float(window);
    }

    toggleFloatingLayer() {
        const floating = [...this.engine.floating];
        const showing = floating.filter(w => !w.minimized);
        if (showing.length) {
            for (const window of showing)
                window.minimize();
            return;
        }
        // The windows minimized out of the strip are floating too while they are minimized
        const minimized = global.display.list_all_windows().filter(w => w.minimized &&
            (this.engine.floating.has(w) || this.engine.info(w)?.fromStrip));
        for (const window of minimized) {
            window.unminimize();
            window.raise();
        }
        const mru = global.display.get_tab_list(Meta.TabList.NORMAL_ALL, null);
        const first = mru.find(w => minimized.includes(w)) ?? minimized[0];
        if (first)
            Main.activateWindow(first);
    }

    // From a floating window every floating window goes away; from anywhere else the one used last comes back
    toggleLastFloating() {
        const floating = [...this.engine.floating];
        if (floating.includes(global.display.focus_window)) {
            for (const window of floating.filter(w => !w.minimized))
                window.minimize();
            return;
        }
        const mru = global.display.get_tab_list(Meta.TabList.NORMAL_ALL, null);
        const last = mru.find(w => floating.includes(w)) ?? floating.find(w => w.minimized);
        if (!last)
            return;
        if (last.minimized)
            last.unminimize();
        Main.activateWindow(last);
    }

    // The keyboard to the window under the pointer
    activateUnderPointer() {
        const [x, y] = global.get_pointer();
        const workspace = global.workspace_manager.get_active_workspace();
        const windows = global.display.sort_windows_by_stacking(workspace.list_windows()).reverse();
        const hit = windows.find(window => {
            const vis = this.engine.info(window)?.vis ?? window.get_frame_rect();
            return window.showing_on_its_workspace() && !this.engine.info(window)?.hidden &&
                x >= vis.x && x < vis.x + vis.width && y >= vis.y && y < vis.y + vis.height;
        });
        if (hit)
            Main.activateWindow(hit);
    }

    // ---- Per workspace ----

    cycleFocusMode() {
        const strip = this.engine.session?.strip ?? this.engine.activeStrip();
        const modes = [Layout.Mode.PLAIN, Layout.Mode.CENTRE, Layout.Mode.EDGE];
        const next = modes[(modes.indexOf(strip.mode) + 1) % modes.length];
        if (next === Layout.Mode.CENTRE && strip.selected) {
            const area = this.engine.area(strip.monitor);
            const vis = this.engine.visual(strip.selected);
            strip.side = vis.x + vis.width / 2 < area.x + area.width / 2 ? -1 : 1;
        }
        this.setFocusMode(strip, next);
    }

    setFocusMode(strip, mode) {
        const leavingCentre = strip.mode === Layout.Mode.CENTRE;
        strip.mode = mode;
        const window = strip.selected;
        if (window) {
            const geo = this.engine.geometry(strip);
            const at = strip.find(window);
            // Out of centring, the window goes back to the side it came from before the edges take over
            if (leavingCentre && at && !geo.centred) {
                const column = geo.columns[at[0]];
                const area = geo.area;
                const last = at[0] === geo.columns.length - 1;
                const left = at[0] === 0 || (!last && strip.side < 0);
                const x = left ? Layout.MARGIN : area.width - Layout.MARGIN - column.width;
                strip.scroll = x - column.x;
                this.engine.info(window).lastX = x;
            }
            this.engine.ensureVisible(strip, window);
        }
        this.tiling.topBar?.sync();
    }

    // Through the positions the settings offer; none offered leaves it as it is
    cycleOpenPosition() {
        const offered = config.openOffered;
        if (!offered.length)
            return;
        this.setOpenPosition(offered[(offered.indexOf(this.tiling.openPosition) + 1) % offered.length]);
    }

    setOpenPosition(value) {
        this.tiling.openPosition = value;
        this.tiling.topBar?.sync();
    }

    // ---- Screens ----

    _neighbour(direction) {
        const from = this.engine.focusMonitor();
        const to = global.display.get_monitor_neighbor_index(from, direction);
        return to >= 0 ? [from, to] : [from, -1];
    }

    // Puts the pointer on another monitor where it was on this one, and shows where it went
    warpTo(monitor) {
        const [x, y] = global.get_pointer();
        const layout = Main.layoutManager.monitors;
        const from = layout[global.display.get_current_monitor()];
        const to = layout[monitor];
        const nx = to.x + Math.round((x - from.x) / from.width * to.width);
        const ny = to.y + Math.round((y - from.y) / from.height * to.height);
        const seat = global.stage.context.get_backend().get_default_seat();
        seat.warp_pointer(nx, ny);
        Main.locatePointer?.show();
    }

    // The screen that way gets the keyboard, and its workspace is GNOME's
    focusMonitor(direction) {
        const [, to] = this._neighbour(direction);
        if (to < 0)
            return;
        this.warpTo(to);
        this.engine.screens.bring(this.engine.screens.shownOn(to), to);
    }

    // The window, with its dialogs, joins the workspace the screen that way shows, and keeps the keyboard there
    moveToMonitor(direction) {
        const window = global.display.focus_window;
        const [, to] = this._neighbour(direction);
        if (!window || to < 0)
            return;
        // A dialog takes its parent, and the parent its dialogs
        const main = window.get_transient_for() ?? window;
        const target = this.engine.activeStrip(to);
        // The layout puts a tiled window on its new screen
        if (this.engine.isTiled(main)) {
            this.engine.insert(main, target);
        } else {
            main.move_to_monitor(to);
            if (!main.is_on_all_workspaces())
                main.change_workspace(target.workspace);
        }
        main.foreach_transient(dialog => {
            dialog.move_to_monitor(to);
            dialog.change_workspace(target.workspace);
            return true;
        });
        this.warpTo(to);
        Main.activateWindow(window);
    }

    // The two screens exchange the workspaces they show; the keyboard and the pointer end on the screen that way
    swapMonitor(direction) {
        const [from, to] = this._neighbour(direction);
        if (to < 0 || !this.engine.screens.swap(from, to))
            return;
        this.warpTo(to);
        const strip = this.engine.activeStrip(to);
        if (strip.selected)
            this.engine.focus(strip);
    }

    // This screen's workspace goes to the screen that way, and this one shows another
    moveSpaceToMonitor(direction) {
        const [from, to] = this._neighbour(direction);
        if (to < 0 || !this.engine.screens.moveSpace(from, to, this.tiling.workspaceHistory()))
            return;
        this.warpTo(to);
        const strip = this.engine.activeStrip(to);
        if (strip.selected)
            this.engine.focus(strip);
    }
}
