// The dock: the pinned apps and folders, the running ones, the last few used, the stacks, the drives, the trash and
// the app grid button, in one row along an edge of the main screen (or of every one). Its strip of the screen is kept
// free, so maximized and tiled windows stop beside it, unless it hides when a window touches it. In the overview it
// stands for GNOME's dash. Its background is the glass, which the glass module draws behind it; on a light theme a
// faint dark veil over the glass keeps the icons clear of a pale wallpaper.
//
// How it moves, what it holds and where it lives are the keys of org.gnome.shell.extensions.amethystora-ui.dock
// (ame desktop dock, Control's Dock settings), read live; a key that is off has nothing connected for it. The parts
// are in dock/: the row (dash.js), one dock per screen (docks.js), the motion under the pointer (swell.js), an app
// starting (launch.js), the marks and numbers (marks.js), recent apps, previews, the fan, folders, stacks, drops,
// the dissolve and hiding.

import GLib from 'gi://GLib';
import Shell from 'gi://Shell';
import St from 'gi://St';

import * as AppFavorites from 'resource:///org/gnome/shell/ui/appFavorites.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import * as SignalTracker from 'resource:///org/gnome/shell/misc/signalTracker.js';

import {Badges} from './dock/badges.js';
import {DockSite} from './dock/docks.js';
import {Dissolve} from './dock/dissolve.js';
import {Drops, PHONE_ID} from './dock/drops.js';
import {Fan} from './dock/fan.js';
import {Folders} from './dock/folders.js';
import {Launch, ringOpacity} from './dock/launch.js';
import {FlatpakUpdates} from './dock/flatpak.js';
import {LauncherEntries} from './dock/launcher.js';
import {AppMenus} from './dock/menus.js';
import {Places} from './dock/places.js';
import {Recent} from './dock/recent.js';
import {Stacks} from './dock/stacks.js';
import {stageRect} from './dock/util.js';

const SCHEMA = 'org.gnome.shell.extensions.amethystora-ui.dock';
// The theme in use, which amethystora-theme puts here
const THEME = GLib.build_filenamev([GLib.get_user_config_dir(), 'amethystora', 'current', 'theme']);
// Lift's peak, which its icons are made at
const LIFT_PEAK = 1.22;
const NUMBERED = 9;

export class Dock {
    constructor(ext) {
        this._ext = ext;
    }

    get ext() {
        return this._ext;
    }

    _syncUpdates() {
        if (this.settings.get_boolean('updating')) {
            this.launcher.updates.start();
        } else {
            this.launcher.updates.stop();
            this.launcher.emit('changed', null);
        }
    }

    enable() {
        this.stopped = false;
        this.settings = this._ext.getSettings(SCHEMA);
        this.motionCount = 0;
        this._frames = {count: 0, sum: 0, max: 0};
        this._sites = [];
        this._numbers = false;
        this._lastPeak = this.peak;

        this.places = new Places();
        this.places.start();
        this.launcher = new LauncherEntries();
        this.launcher.start();
        this.launcher.updates = new FlatpakUpdates(id => this.launcher.emit('changed', id));
        this._syncUpdates();
        this.badges = new Badges(this.launcher);
        this.badges.start();
        // GNOME's app menus pin to the dock, by name, wherever they open
        this._menus = new AppMenus(this.launcher);
        this._menus.enable();
        this.folders = new Folders(this);
        this.folders.start();
        this.stacks = new Stacks(this);
        this.stacks.start();
        this.drops = new Drops(this);
        this.drops.start();
        this.dissolve = new Dissolve(this);
        this.fan = new Fan(this);
        this._syncLaunch();
        this._syncRecent();

        // GNOME's dash gives the overview's bottom row to the dock: hidden, but as tall as the dock, so that the
        // overview leaves the dock its room
        this._overviewDash = Main.overview.dash;
        this._overviewDash.hide();

        const comfort = this._ext.modules?.get('comfort');
        this._unlisten = comfort?.listen?.(() => this._motionChanged()) ?? null;
        St.Settings.get().connectObject('notify::enable-animations', () => this._motionChanged(),
            'notify::color-scheme', () => this._syncScheme(), this);
        this.settings.connectObject(
            'changed::magnify', () => this._syncMagnify(),
            'changed::magnify-amount', () => this._syncMagnify(),
            'changed::launch', () => this._syncLaunch(),
            'changed::indicator', () => this._each(dash => dash.remark()),
            'changed::recent', () => this._syncRecent(),
            'changed::previews', () => this._each(dash => dash.previews?.hide(true)),
            'changed::edge', () => this._rebuild(),
            'changed::screens', () => this._rebuild(),
            'changed::autohide', () => this._rebuild(),
            'changed::workspace-apps', () => this._syncFilters(),
            'changed::updating', () => this._syncUpdates(),
            'changed::icon-size', () => this._syncIconSize(),
            this);
        Main.layoutManager.connectObject('monitors-changed', () => this._rebuild(), this);
        global.display.connectObject('workareas-changed', () => this._sites.forEach(site => site.place()), this);
        // The theme's stylesheets loaded again: a theme switch
        St.ThemeContext.get_for_stage(global.stage).connectObject('changed', () => this._syncScheme(), this);
        this._build();
    }

    disable() {
        this.stopped = true;
        if (this._unclipId)
            GLib.source_remove(this._unclipId);
        this._unclipId = 0;
        this._unlisten?.();
        this._unlisten = null;
        this.settings?.disconnectObject(this);
        St.Settings.get().disconnectObject(this);
        Main.layoutManager.disconnectObject(this);
        global.display.disconnectObject(this);
        global.workspace_manager.disconnectObject(this);
        for (const window of this._workspaceWatched ?? [])
            window.disconnectObject(this);
        this._workspaceWatched = null;
        St.ThemeContext.get_for_stage(global.stage).disconnectObject(this);
        this.fan?.close(true);
        this.fan = null;
        this._card?.close(true);
        this._card = null;
        // The launch halos are drawn in the docks' own containers, so they go before the docks do
        this.launch?.stop();
        this.launch = null;
        this._destroyDocks();
        this.dissolve?.stop();
        this.dissolve = null;
        this.drops?.stop();
        this.drops = null;
        this.stacks?.stop();
        this.stacks = null;
        this.folders?.stop();
        this.folders = null;
        this.recent?.stop();
        this.recent = null;

        this._menus?.disable();
        this._menus = null;

        if (this._overviewDash) {
            this._overviewDash.height = -1;
            this._overviewDash.show();
        }
        this._overviewDash = null;

        this.badges?.stop();
        this.badges = null;
        this.launcher?.updates?.stop();
        this.launcher?.stop();
        this.launcher = null;
        this.places?.stop();
        this.places = null;
        // Disposed here, on the main thread, rather than left to the collector: GJS sweeping a settings object
        // while dconf's worker dispatches a change deadlocks the shell (its toggle queue against the backend's lock)
        this.settings?.run_dispose();
        this.settings = null;
        this._ext.modules?.get('glass')?.surfacesChanged?.();
    }

    // ---- The docks ----

    get docks() {
        return this._sites;
    }

    // Each screen its own dock while there are several and the key says every
    get perScreen() {
        return this.settings?.get_string('screens') === 'every' && Main.layoutManager.monitors.length > 1;
    }

    get workspaceApps() {
        return this.settings?.get_boolean('workspace-apps') ?? false;
    }

    get indicator() {
        return this.settings?.get_string('indicator') ?? 'bar';
    }

    get phoneId() {
        return PHONE_ID;
    }

    // The size the icons are made at, as a multiple of their own: what the dock grows them to
    get peak() {
        const magnify = this.settings?.get_string('magnify');
        if (magnify === 'swell')
            return this.settings.get_double('magnify-amount');
        return magnify === 'lift' ? LIFT_PEAK : 1;
    }

    _build() {
        const monitors = Main.layoutManager.monitors;
        if (!Main.layoutManager.primaryMonitor)
            return;
        const edge = this.settings.get_string('edge');
        const autohide = this.autohide;
        const indices = this.perScreen ? monitors.map((m, i) => i) : [Main.layoutManager.primaryIndex];
        for (const index of indices) {
            const site = new DockSite(this, index, edge, autohide);
            this._sites.push(site);
            site.build();
            site.dash.swell.setMode(this.settings.get_string('magnify'), this.settings.get_double('magnify-amount'));
        }
        this._syncFilters();
        this._syncScheme();
        this.glassChanged();
        if (this._numbers)
            this.showNumbers({reveal: this._numbersReveal});
    }

    _destroyDocks() {
        for (const site of this._sites)
            site.destroy();
        this._sites = [];
    }

    // When the dock gets out of the way: the key, or what a focus profile lays over it while it is on
    get autohide() {
        return this._autohideOverride ?? this.settings?.get_string('autohide') ?? 'off';
    }

    // A focus profile's autohide ('touch'), or null for the key's: in memory, the key is never written
    setAutohideOverride(value) {
        const before = this.autohide;
        this._autohideOverride = value === 'touch' ? 'touch' : null;
        if (this.settings && this.autohide !== before)
            this._rebuild();
    }

    // Touch mode (modules/touch.js): icons up to GNOME's next size, in memory
    setTouch(on) {
        if (!!on === !!this.touchMode)
            return;
        this.touchMode = !!on;
        if (this.settings)
            this._rebuild();
    }

    // The icons' size a person picked (R6-28), or null for GNOME's own choice as the image ships
    get iconSize() {
        if (!this.settings || this.settings.get_user_value('icon-size') === null)
            return null;
        return Math.min(64, Math.max(40, this.settings.get_int('icon-size')));
    }

    // A new size: each dock measured again, at once, so that a slider moved in Control is followed as it moves
    _syncIconSize() {
        for (const site of this._sites) {
            site.place();
            site.dash?._queueRedisplay?.();
        }
    }

    // A screen added, taken away or rearranged, or another edge: the docks are made again
    _rebuild() {
        this.fan?.close(true);
        this._card?.close(true);
        this._card = null;
        this._destroyDocks();
        this._build();
    }

    _each(fn) {
        for (const site of this._sites) {
            if (site.dash)
                fn(site.dash, site);
        }
    }

    // A dock was placed: the overview leaves room for the main screen's bottom dock
    placed(site) {
        // A bar at the bottom keeps the room of the dock above it too
        this._ext.modules?.get('bars')?.dockPlaced?.();
        if (!this._overviewDash || site.monitorIndex !== Main.layoutManager.primaryIndex)
            return;
        this._overviewDash.height = site.edgeName === 'bottom' && !site.touch ? site.thickness : 0;
    }

    // The workspace a screen shows: the tiling's own with its separate screens, else GNOME's
    shownOn(monitor) {
        const screens = this._ext.modules?.get('tiling')?.engine?.screens;
        try {
            return screens?.shownOn?.(monitor) ?? global.workspace_manager.get_active_workspace();
        } catch {
            return global.workspace_manager.get_active_workspace();
        }
    }

    // The tiling's screens changed what they show
    screensChanged() {
        if (this.workspaceApps || this.perScreen)
            this._refilter();
    }

    _refilter() {
        this._each(dash => {
            dash._queueRedisplay();
            dash._syncAll();
        });
    }

    // Only what the filters need: window moves between screens with several docks, workspaces with workspace-apps
    _syncFilters() {
        global.display.disconnectObject(this);
        global.workspace_manager.disconnectObject(this);
        global.display.connectObject('workareas-changed', () => this._sites.forEach(site => site.place()), this);
        this._filterSignals = 0;
        if (this.perScreen) {
            global.display.connectObject(
                'window-entered-monitor', () => this._refilter(),
                'window-left-monitor', () => this._refilter(), this);
            this._filterSignals += 2;
        }
        if (this.workspaceApps) {
            global.workspace_manager.connectObject('active-workspace-changed', () => this._refilter(), this);
            global.display.connectObject('window-created', (d, window) => this._watchWorkspace(window), this);
            for (const actor of global.get_window_actors())
                this._watchWorkspace(actor.meta_window);
            this._filterSignals += 2;
        } else {
            for (const window of this._workspaceWatched ?? [])
                window.disconnectObject(this);
            this._workspaceWatched = new Set();
        }
        this._refilter();
    }

    _watchWorkspace(window) {
        this._workspaceWatched ??= new Set();
        if (this._workspaceWatched.has(window))
            return;
        this._workspaceWatched.add(window);
        window.connectObject('workspace-changed', () => this._refilter(),
            'unmanaged', () => this._workspaceWatched?.delete(window), this);
    }

    // ---- Keys ----

    _syncMagnify() {
        const peak = this._lastPeak;
        this._lastPeak = this.peak;
        this._each(dash => {
            dash.swell.setMode(this.settings.get_string('magnify'), this.settings.get_double('magnify-amount'));
            if (peak !== this.peak)
                dash.reloadIcons();
        });
    }

    _syncLaunch() {
        const mode = this.settings.get_string('launch');
        if (mode === 'off') {
            this.launch?.stop();
            this.launch = null;
            return;
        }
        if (!this.launch) {
            this.launch = new Launch(this);
            this.launch.start(mode);
        } else {
            this.launch.setMode(mode);
        }
    }

    _syncRecent() {
        const on = this.settings.get_boolean('recent');
        if (on && !this.recent) {
            this.recent = new Recent();
            this.recent.start();
            this._each(dash => this.recent.connectObject('changed', () => dash._queueRedisplay(), dash));
        } else if (!on && this.recent) {
            this.recent.stop();
            this._each(dash => this.recent.disconnectObject(dash));
            this.recent = null;
        }
        this._each(dash => dash._queueRedisplay());
    }

    // How the dock moves: full, reduced (comfort's reduce motion: fades in place) or none (animations off)
    motionLevel() {
        if (!St.Settings.get().enable_animations)
            return 'none';
        return this._ext.modules?.get('comfort')?.reduceMotion ? 'reduced' : 'full';
    }

    _motionChanged() {
        this._each(dash => dash.swell.sync());
    }

    // The veil is for a light theme's pale wallpaper, whichever style the shell is in (Dark Style may be on over a
    // light theme); before any theme has been applied the shell's style stands in. Looked at again whenever the
    // shell's style changes or the theme's stylesheets are loaded again, as a theme switch does.
    _syncScheme() {
        const themed = GLib.file_test(THEME, GLib.FileTest.IS_DIR);
        const light = themed ? GLib.file_test(GLib.build_filenamev([THEME, 'light.mode']), GLib.FileTest.EXISTS)
            : Main.getStyleVariant() === 'light';
        this._each(dash => {
            if (light)
                dash.add_style_class_name('amethystora-dock-light');
            else
                dash.remove_style_class_name('amethystora-dock-light');
        });
    }

    // ---- What the parts tell each other ----

    get cardOpen() {
        return Boolean(this.fan?.open || this._card?.isOpen);
    }

    get dragging() {
        return Boolean(this._dragging || this.drops?.dragging);
    }

    dragChanged(on) {
        if (on !== undefined)
            this._dragging = on;
        this._each(dash => {
            dash.swell.sync();
            if (this.dragging)
                dash.previews?.hide(true);
        });
    }

    menuChanged(dash, open) {
        for (const site of this._sites) {
            if (site.dash === dash)
                site.autohide?.hold('menu', open);
        }
    }

    // A stack's or folder's card: a click on its item again closes it
    openCard(dash, state) {
        if (!state)
            return null;
        if (this._card && this._card.state === state) {
            this._card.close();
            return null;
        }
        this._card?.close(true);
        this._card = null;
        dash.previews?.hide(true);
        if (state.key.stack)
            this._card = this.stacks.show(dash, state);
        else if (state.key.folder)
            this._card = this.folders.show(dash, state, state.key.folder);
        for (const site of this._sites)
            site.autohide?.hold('card', Boolean(this._card));
        this._each(other => other.swell.sync());
        // The card is the dock's now: the glass draws behind it
        this.glassChanged();
        return this._card;
    }

    cardClosed(popover) {
        if (this._card !== popover)
            return;
        this._card = null;
        for (const site of this._sites)
            site.autohide?.hold('card', false);
        this._each(dash => dash.swell.sync());
    }

    glassChanged() {
        this._ext.modules?.get('glass')?.surfacesChanged?.();
    }

    rowChanged() {
        if (this._numbers && !this._renumbering) {
            this._renumbering = true;
            try {
                this.showNumbers({reveal: this._numbersReveal, again: true});
            } finally {
                this._renumbering = false;
            }
        }
    }

    noteFrame(ms) {
        const frames = this._frames;
        frames.count++;
        frames.sum += ms;
        frames.max = Math.max(frames.max, ms);
    }

    // Every dock item of an app, on every dock
    itemsFor(app) {
        const states = [];
        this._each(dash => {
            for (const state of dash._items.values()) {
                if (state.key === app && !state.underFolder)
                    states.push(state);
            }
        });
        return states;
    }

    // ---- For the glass ----

    // The pills, with the corners their stylesheet gives them (the desktop style's), and the cards while shown
    glassSurfaces() {
        const surfaces = [];
        this._each(dash => {
            surfaces.push({actor: dash.pill, material: 'dock', radius: null});
            if (dash.previews?.card)
                surfaces.push({actor: dash.previews.card, material: 'glass', radius: null});
        });
        for (const tile of this.fan?.surfaces ?? [])
            surfaces.push({actor: tile, material: 'glass', radius: null});
        // Only while open: a card closing at once (animations off) is destroyed before the dock lets go of it
        if (this._card?.isOpen)
            surfaces.push({actor: this._card.card, material: 'glass', radius: null});
        if (this.drops?.card)
            surfaces.push({actor: this.drops.card, material: 'glass', radius: null});
        return surfaces;
    }

    // ---- For the motion module ----

    // The resting box of the icon picture of the item that holds a window, on the window's screen
    iconBoxFor(window) {
        const app = Shell.WindowTracker.get_default().get_window_app(window);
        const owner = this.places?.owner(window);
        return this._iconBox(owner ?? app, window.get_monitor());
    }

    iconBoxForApp(app, monitor) {
        return this._iconBox(app, monitor);
    }

    _iconBox(key, monitor) {
        if (!key)
            return null;
        const site = this._sites.find(s => s.monitorIndex === monitor) ?? this._sites[0];
        if (!site?.dash || site.autohide?.hidden)
            return null;
        for (const state of site.dash._items.values()) {
            if (state.key === key && !state.underFolder)
                return site.dash.iconRestRect(state);
        }
        // An app in a folder: the folder's picture
        const id = key.get_id?.();
        const folder = id ? this.folders?.folderOf(id) : null;
        if (folder) {
            for (const state of site.dash._items.values()) {
                if (state.key.folder?.name === folder.name)
                    return site.dash.iconRestRect(state);
            }
        }
        return null;
    }

    // ---- The numbers, for the windows module (board 16) ----

    showNumbers({reveal = false, again = false} = {}) {
        this._numbers = true;
        if (this._unclipId)
            GLib.source_remove(this._unclipId);
        this._unclipId = 0;
        this._numbersReveal = reveal;
        const favorites = AppFavorites.getAppFavorites().getFavorites().slice(0, NUMBERED);
        const level = this.motionLevel();
        this._each((dash, site) => {
            if (reveal)
                site.autohide?.hold('numbers', true);
            if (site.autohide?.hidden && !reveal)
                return;
            dash.swell.unclip(true);
            let order = 0;
            const shown = new Set();
            favorites.forEach((app, i) => {
                const state = [...dash._items.values()].find(s => s.key === app && !s.underFolder &&
                    s.item.visible && s.item.get_parent());
                if (!state)
                    return;
                shown.add(state);
                if (again && state.marks?.numberShown && state.marks.number === i + 1)
                    return;
                state.marks?.showNumber(i + 1, again ? 0 : order++, again ? 'none' : level);
            });
            for (const state of dash._items.values()) {
                if (!shown.has(state))
                    state.marks?.hideNumber();
            }
        });
    }

    hideNumbers() {
        this._numbers = false;
        this._each((dash, site) => {
            for (const state of dash._items.values())
                state.marks?.hideNumber();
            site.autohide?.hold('numbers', false);
        });
        // The row is cut to its box again once the discs have faded
        if (this._unclipId)
            GLib.source_remove(this._unclipId);
        this._unclipId = GLib.timeout_add(GLib.PRIORITY_DEFAULT, 200, () => {
            this._unclipId = 0;
            if (!this._numbers)
                this._each(dash => dash.swell.unclip(false));
            return GLib.SOURCE_REMOVE;
        });
    }

    get numbersShown() {
        return this._numbers;
    }

    numberedItems() {
        const favorites = AppFavorites.getAppFavorites().getFavorites().slice(0, NUMBERED);
        const items = [];
        favorites.forEach((app, i) => {
            let drawn = false;
            this._each(dash => {
                const state = [...dash._items.values()].find(s => s.key === app && !s.underFolder);
                const box = state ? dash.iconRestRect(state) : null;
                if (!state || !box)
                    return;
                drawn = true;
                items.push({
                    number: i + 1, appId: app.get_id(), monitor: dash.monitorIndex,
                    x: box.x + box.width / 2, y: box.y + box.height / 2, disc: state.marks?.discBox() ?? null,
                });
            });
            if (!drawn)
                items.push({number: i + 1, appId: app.get_id(), monitor: -1, x: null, y: null, disc: null});
        });
        return items;
    }

    // ---- A read-only state for tests (spec 4.16) ----

    state() {
        const favorites = AppFavorites.getAppFavorites().getFavorites().map(app => app.get_id());
        const docks = this._sites.map(site => {
            const dash = site.dash;
            if (!dash)
                return {monitor: site.monitorIndex, edge: site.edgeName, items: []};
            const pill = dash.pillRect();
            const swellItems = dash.swell.items;
            const scaleLong = dash.edge.long === 'x' ? dash.pill.scale_x : dash.pill.scale_y;
            const area = Main.layoutManager.getWorkAreaForMonitor(site.monitorIndex);
            const items = swellItems.map(entry => {
                const container = entry.container;
                const state = dash._items.get(entry.actor);
                const key = state?.key;
                let kind = 'grid';
                if (container !== dash._showAppsIcon) {
                    if (entry.separator)
                        kind = 'divider';
                    else if (container._amethystoraRecent)
                        kind = 'recent';
                    else if (key?.folder)
                        kind = 'folder';
                    else if (key?.stack)
                        kind = 'stack';
                    else if (key instanceof Shell.App)
                        kind = 'app';
                    else
                        kind = 'place';
                }
                const id = key instanceof Shell.App ? key.get_id() : key?.name ?? null;
                const rest = dash.restRectOf(container);
                const icon = state?.button.icon?.icon;
                return {
                    id, kind,
                    pinned: key instanceof Shell.App ? favorites.includes(key.get_id()) : kind === 'folder',
                    number: key instanceof Shell.App && favorites.indexOf(key.get_id()) < 9
                        ? favorites.indexOf(key.get_id()) + 1 : 0,
                    rest, scale: entry.actor.scale_x,
                    translation: dash.edge.long === 'x' ? entry.actor.translation_x : entry.actor.translation_y,
                    translationAway: dash.edge.long === 'x' ? entry.actor.translation_y : entry.actor.translation_x,
                    shown: stageRect(entry.actor),
                    mark: state?.marks?.markBox ?? null,
                    ringOpacity: state && this.launch ? ringOpacity(this.launch, state) : 0,
                    previewsShown: Boolean(dash.previews?.shown && dash.previews._state === state),
                    iconTexture: icon?._amethystoraDrawn ?? icon?.icon_size ?? null,
                    iconBin: state ? {
                        translation: state.button.icon?._iconBin?.translation_y ?? 0,
                        translationX: state.button.icon?._iconBin?.translation_x ?? 0,
                        opacity: state.button.icon?._iconBin?.opacity ?? 255,
                        scale: state.button.icon?._iconBin?.scale_x ?? 1,
                    } : null,
                };
            });
            const strip = site.stripRect();
            return {
                monitor: site.monitorIndex,
                edge: site.edgeName,
                strut: site.reserves ? {side: site.edgeName, ...strip} : null,
                touch: site.touch,
                hidden: Boolean(site.autohide?.hidden),
                barrier: Boolean(site.autohide?.hasBarrier),
                strip: {
                    ...strip, translationX: site.strip?.translation_x ?? 0,
                    translationY: site.strip?.translation_y ?? 0,
                },
                workArea: {x: area.x, y: area.y, width: area.width, height: area.height},
                pill: {...pill, scaleLong},
                pitch: dash.swell._pitch ?? 0,
                gaps: dash.swell._gaps ?? [],
                items,
                caption: dash._caption ? {opacity: dash._caption.opacity, visible: dash._caption.visible} : null,
            };
        });
        const frames = this._frames;
        return {
            docks,
            magnify: this.settings.get_string('magnify'),
            amount: this.settings.get_double('magnify-amount'),
            motionLevel: this.motionLevel(),
            motionCount: this.motionCount,
            frameClockRunning: this._sites.some(site => site.dash?.swell.running),
            frameMs: {count: frames.count, avg: frames.count ? frames.sum / frames.count : 0, max: frames.max},
            connections: this._connections(),
            recent: this.recent ? this.recent.shownFor([]).map(app => app.get_id()) : [],
            recentShown: this._sites[0]?.dash ? [...this._sites[0].dash._recentItems.keys()].map(app => app.get_id())
                : [],
            folders: this.folders?.value ?? {},
            stacks: this.stacks?.values ?? [],
            lastLaunch: this.launch?.last ?? null,
            numbersShown: this.numbersShown,
            previews: (() => {
                const dash = this._sites.find(site => site.dash?.previews?.shown)?.dash;
                return {
                    shown: Boolean(dash), windows: dash?.previews.windows ?? 0,
                    card: dash ? stageRect(dash.previews.card) : null,
                };
            })(),
            fan: {open: Boolean(this.fan?.open), windows: this.fan?.windows.length ?? 0, ringed: this.fan?.ringed ?? 0},
            stackCard: {
                open: Boolean(this._card?.isOpen && this._card.state.key.stack),
                entries: this._card?.state.key.stack ? this.stacks.entries ?? [] : [],
            },
            folderCard: {open: Boolean(this._card?.isOpen && this._card.state.key.folder)},
            stackReads: this.stacks?.reads ?? 0,
            lastDrop: this.drops?.last ?? null,
            dissolves: this.dissolve?.count ?? 0,
            dissolving: Boolean(this.dissolve?.playing),
        };
    }

    // The signal handlers the dock and its parts hold through GNOME's signal trackers, counted handler by handler,
    // for tests that switch every key off and on again
    _connections() {
        const trackers = SignalTracker.debugGetSignalTrackers?.();
        if (!trackers)
            return -1;
        const owners = [this, this.launch, this.recent, this.drops, this.folders, this.stacks, this.fan, this.dissolve,
            ...this._sites.flatMap(site => [site, site.autohide, site.dash?.swell, site.dash?.previews])];
        // GNOME keeps a tracker for each object that emits, holding each owner's handlers on it
        const mine = new Set(owners.filter(Boolean));
        let count = 0;
        for (const tracker of trackers.values()) {
            for (const [owner, data] of tracker._map ?? []) {
                if (mine.has(owner))
                    count += data.ownerSignals?.length ?? 0;
            }
        }
        return count;
    }
}
