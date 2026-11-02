// The top bar on every screen, and the top bar over a fullscreen window. With two screens or more and every-screen on
// (it is off as the image ships), each screen but the main one gets a bar of its own at its top, drawn as the main bar
// is (the same names and classes, so the desktop style and the glass give it the same shape), holding two things: the workspaces, the one that screen shows lit,
// and the clock. A click on a workspace shows it on that screen, swapping with the screen that showed it. The tray,
// quick settings and the menus stay on the main screen. In the classic layout, while GNOME keeps the other screens'
// windows on every workspace, the bar holds the clock only. A bar the tiling hides on a workspace (Super+Ctrl+B) is
// hidden on the screen that shows it, and gives its height back.
//
// While a window is fullscreen, pushing the pointer against the top edge of its screen (as hard as GNOME's hot corner
// asks) brings that screen's bar down over it; it goes again once the pointer has been away from it for a moment.
//
// The bar your way: what it holds and where (bars/arrange.js, the layout key), its size (compact, regular or roomy: a
// class on the shell's top group that bars.css sizes the bar by), and its edge: at the bottom, its menus opening upwards
// and a dock at the bottom lifted above it, or hidden until the pointer pushes against its edge, the peek below brought
// in for it on every screen and not only over fullscreen.
//
// All are keys of org.gnome.shell.extensions.amethystora-ui.bars (ame desktop top-bar, Control), read live. Nothing
// here polls: the bars follow GNOME's signals and the tiling's screens, and a barrier exists only while a window is
// fullscreen and the peek is on, or while the bar hides until touched.
//
// For the tests only (tests/vm/suites/19-top-bars.sh), entry points that read what a person sees: bars() lists the
// secondary bars ({monitor, actor, activeIndex(), clock(), parts(), labels()}), peeking(monitor) says whether that
// screen's bar is peeking.

import Clutter from 'gi://Clutter';
import GLib from 'gi://GLib';
import GnomeDesktop from 'gi://GnomeDesktop';
import Gio from 'gi://Gio';
import Graphene from 'gi://Graphene';
import Meta from 'gi://Meta';
import Shell from 'gi://Shell';
import St from 'gi://St';

import {gettext as _} from 'resource:///org/gnome/shell/extensions/extension.js';
import * as Layout from 'resource:///org/gnome/shell/ui/layout.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import * as PanelMenu from 'resource:///org/gnome/shell/ui/panelMenu.js';

import {Arrange} from './bars/arrange.js';
import {freeBottomEdge, freeTopEdge} from './bars/edge.js';

const SCHEMA = 'org.gnome.shell.extensions.amethystora-ui.bars';
const TILING = 'org.gnome.shell.extensions.amethystora-ui.tiling';
// GNOME keeps the other screens' windows on every workspace while this is true (its default, which the tiling turns
// off while it runs)
const MUTTER = 'org.gnome.mutter';
const ONLY_PRIMARY = 'workspaces-only-on-primary';
// How hard the pointer has to push against the top edge, as GNOME's hot corner asks: this many pixels within this
// many milliseconds
const PRESSURE = 100;
const PRESSURE_MS = 1000;
// The bar coming down and going back up, before GNOME's slow-down factor, and its curve: quick, then settling
const SLIDE_MS = 260;
const CURVE = [new Graphene.Point({x: 0.2, y: 0.8}), new Graphene.Point({x: 0.2, y: 1})];
// How long the pointer is away from a peeking bar before it goes
const AWAY_MS = 700;
// How many more times the pointer is looked for on a peeking bar once Clutter has said it was there
const RECHECKS = 3;
// The classes on the shell's top group: the bar's size (regular has none) and the bar at the bottom
const SIZES = {compact: 'amethystora-bar-compact', roomy: 'amethystora-bar-roomy'};
const BOTTOM = 'amethystora-bar-bottom';

// The connector of each screen, by GNOME's monitor number, for a screen plugged back to find its bar again
function connectors() {
    const list = [];
    for (const logical of global.backend.get_monitor_manager().get_logical_monitors())
        list[logical.get_number()] = logical.get_monitors()[0]?.get_connector() ?? null;
    return list;
}

function easeCurved(actor, props, duration, onStopped) {
    actor.ease({...props, duration, mode: Clutter.AnimationMode.EASE_OUT_QUAD, onStopped});
    for (const name of Object.keys(props))
        actor.get_transition(name.replaceAll('_', '-'))?.set_cubic_bezier_progress(...CURVE);
}

// A plain panel button, as GNOME's top bar has them, that answers nothing: the clock, the workspaces' frame
function plainButton(name) {
    const button = new PanelMenu.Button(0.5, name, true);
    button.reactive = false;
    button.can_focus = false;
    button.track_hover = false;
    return button;
}

// One screen's bar: a box at the screen's top that GNOME's layout tracks like the main bar's box, and in it a panel
// with GNOME's names, so that every rule for the main bar applies
class Bar {
    constructor(module, index, connector) {
        this._module = module;
        this.connector = connector;
        this.monitor = index;
        this.row = null;

        this.box = new St.BoxLayout({
            style_class: 'amethystora-bars-box',
            orientation: Clutter.Orientation.VERTICAL,
        });
        // On the stage before anything is put in it, so that nothing asks for its style while it is off it
        Main.layoutManager.addChrome(this.box, {affectsStruts: true, trackFullscreen: true});
        this.panel = new St.BoxLayout({
            name: 'panel',
            style_class: 'amethystora-bars-secondary',
            reactive: true,
            x_expand: true,
        });
        this.left = new St.BoxLayout({name: 'panelLeft', x_expand: true});
        this.center = new St.BoxLayout({name: 'panelCenter'});
        this.right = new St.BoxLayout({name: 'panelRight'});
        this.panel.add_child(this.left);
        this.panel.add_child(this.center);
        this.panel.add_child(this.right);
        this.box.add_child(this.panel);

        this.workspaces = plainButton(_('Workspaces'));
        this.workspaces.add_style_class_name('amethystora-workspaces-button');
        this.left.add_child(this.workspaces.container);

        this.clock = plainButton(_('Clock'));
        this.clock.add_style_class_name('clock-display');
        const clockBox = new St.BoxLayout({style_class: 'clock-display-box'});
        this.time = new St.Label({style_class: 'clock', y_align: Clutter.ActorAlign.CENTER});
        this.time.clutter_text.use_markup = false;
        clockBox.add_child(this.time);
        this.clock.add_child(clockBox);
        this.clock.label_actor = this.time;
        // In the middle box, as the main bar's clock is, so that a style that draws the clock its own way (capsule,
        // neon) draws this one so too; with the right box empty it sits at the end of the bar
        this.center.add_child(this.clock.container);

        if (Main.overview.visible)
            this.panel.add_style_pseudo_class('overview');
        // At the bottom, its place depends on its height
        this.box.connect('notify::height', () => {
            if (this._module.bottom)
                this.place(this.monitor);
        });
        this.place(index);
    }

    place(index) {
        this.monitor = index;
        const monitor = Main.layoutManager.monitors[index];
        if (!monitor)
            return;
        const y = this._module.bottom ? monitor.y + monitor.height - this.box.height : monitor.y;
        this.box.set_position(monitor.x, y);
        this.box.set_size(monitor.width, -1);
    }

    // The workspaces, or none (the clock alone)
    setRow(wanted) {
        if (wanted && !this.row) {
            this.row = this._module.workspacesModule?.rowFor(this.monitor) ?? null;
            if (this.row)
                this.workspaces.add_child(this.row.actor);
        } else if (!wanted && this.row) {
            this.row.destroy();
            this.row = null;
        }
        if (this.row)
            this.row.monitor = this.monitor;
        this.workspaces.container.visible = !!this.row;
    }

    destroy() {
        this.row?.destroy();
        this.row = null;
        // Destroyed where it is: GNOME's layout lets go of a tracked actor as it is destroyed, and its children go while
        // they are still on the stage
        this.box.destroy();
    }
}

export class Bars {
    constructor(ext) {
        this._ext = ext;
    }

    get workspacesModule() {
        return this._ext.modules?.get('workspaces') ?? null;
    }

    get _screens() {
        return this._ext.modules?.get('tiling')?.engine?.screens ?? null;
    }

    enable() {
        // connector -> Bar
        this._bars = new Map();
        // monitor index -> {barrier, pressure}
        this._barriers = new Map();
        // monitor index -> what the peek of that screen's bar holds
        this._peeks = new Map();
        this._settings = this._ext.getSettings(SCHEMA);
        // monitor index -> the transparent strip that keeps the room of a bottom bar and a dock above it
        this._spacers = new Map();
        // The menus this module turned upwards, and the struts it took away while the bar hides
        this._turned = new Set();
        this._strutless = new Set();
        this._settings.connectObject(
            'changed::every-screen', () => this._queueSync(),
            'changed::peek', () => this._syncPeek(),
            'changed::size', () => this._syncSize(),
            'changed::position', () => this._syncPosition(),
            'changed::autohide', () => this._syncPosition(),
            this);
        this._syncSize();
        this._mutter = new Gio.Settings({schema_id: MUTTER});
        this._mutter.connectObject(`changed::${ONLY_PRIMARY}`, () => this._queueSync(), this);
        // The tiling starts after this module and stops and starts with its own switch: its screens are looked for
        // again after it did
        this._tiling = this._ext.getSettings(TILING);
        this._tiling.connectObject('changed::enabled', () => this._queueSync(), this);
        Main.layoutManager.connectObject('monitors-changed', () => {
            this._queueSync();
            this._rebuildBarriers();
        }, this);
        global.display.connectObject('in-fullscreen-changed', () => this._syncPeek(), this);
        Main.overview.connectObject(
            'showing', () => this._eachBar(bar => bar.panel.add_style_pseudo_class('overview')),
            'hiding', () => this._eachBar(bar => bar.panel.remove_style_pseudo_class('overview')),
            this);
        this._wrapVisibility();
        this._wrapBoxes();
        this._sync();
        this._queueSync();
        this._syncPosition();
        this._arrange = new Arrange(this._settings, moved => {
            if (moved)
                this._told();
            this._syncMenus();
        });
        this._arrange.enable();
    }

    disable() {
        this._arrange?.disable();
        this._arrange = null;
        this._settings?.disconnectObject(this);
        // Disposed now rather than by the collector (dock.js says why)
        this._settings?.run_dispose();
        this._settings = null;
        this._mutter?.disconnectObject(this);
        this._mutter?.run_dispose();
        this._mutter = null;
        this._tiling?.disconnectObject(this);
        this._tiling?.run_dispose();
        this._tiling = null;
        Main.layoutManager.disconnectObject(this);
        global.display.disconnectObject(this);
        Main.overview.disconnectObject(this);
        if (this._later)
            global.compositor.get_laters().remove(this._later);
        this._later = 0;
        this._hookScreens(null);

        for (const index of [...this._peeks?.keys() ?? []])
            this._endPeek(index);
        this._peeks = null;
        for (const index of [...this._barriers?.keys() ?? []])
            this._dropBarrier(index);
        this._barriers = null;
        this._unwrapVisibility();
        // Back at the top, the size as GNOME has it, every strut given back, every menu opening downwards
        this._bottom = false;
        this._autohide = false;
        this._unwrapBoxes();
        Main.layoutManager.panelBox.disconnectObject(this);
        Main.layoutManager._updateBoxes?.();
        for (const actorData of this._strutless ?? [])
            actorData.affectsStruts = true;
        this._strutless = null;
        for (const spacer of this._spacers?.values() ?? [])
            spacer.destroy();
        this._spacers = null;
        this._syncMenus();
        this._turned = null;
        const group = Main.layoutManager.uiGroup;
        for (const name of [...Object.values(SIZES), BOTTOM])
            group.remove_style_class_name(name);

        // The others let go of the bars before they go
        const gone = [...this._bars?.values() ?? []];
        this._bars = null;
        if (gone.length)
            this._told();
        for (const bar of gone)
            bar.destroy();
        this._setClock(false);
        Main.layoutManager._updateVisibility?.();
        Main.layoutManager._queueUpdateRegions?.();
        this._ext.modules?.get('dock')?.docks?.forEach?.(site => site.place());
    }

    // ---- For the other modules ----

    // The secondary bars' three boxes, for the desktop style to mark their buttons (modules/style.js)
    boxes() {
        return [...this._bars?.values() ?? []].flatMap(bar => [bar.left, bar.center, bar.right]);
    }

    // The secondary bars' panels and their screens, for the glass (glass/topbar.js)
    panels() {
        return [...this._bars?.values() ?? []].map(bar => ({panel: bar.panel, monitor: bar.monitor, text: () => bar.clock}));
    }

    // The box of the bar on a screen, shown or not, or null (tiling/topbar.js draws its line under it)
    barOn(monitor) {
        for (const bar of this._bars?.values() ?? []) {
            if (bar.monitor === monitor)
                return bar.box;
        }
        return null;
    }

    // Whether the bars sit at the bottom of their screens
    get bottom() {
        return !!this._bottom;
    }

    // How much a dock at the bottom of a screen is lifted: the height of a bar at the bottom there that keeps its room
    bottomInset(monitor) {
        if (!this._bottom || this._autohide || !this._bars)
            return 0;
        const bar = this._barOf(monitor);
        return bar && (bar.main || bar.box.visible) ? Math.round(bar.box.height) : 0;
    }

    // A dock was placed (modules/dock.js): the room kept for it under a bottom bar follows
    dockPlaced() {
        this._syncSpacers();
    }

    // ---- For the tests ----

    // The main bar's items in each box and hidden, as arranged (bars/arrange.js)
    get placed() {
        return this._arrange?.placed ?? null;
    }

    spacer(monitor) {
        return this._spacers?.get(monitor) ?? null;
    }

    bars() {
        return [...this._bars?.values() ?? []].sort((a, b) => a.monitor - b.monitor).map(bar => ({
            monitor: bar.monitor,
            actor: bar.box,
            activeIndex: () => bar.row?.activeIndex() ?? -1,
            clock: () => bar.time.text,
            // What the bar holds, by the part of the main bar each is: only workspaces and the clock
            parts: () => [bar.left, bar.center, bar.right].flatMap(box => box.get_children())
                .filter(c => c.visible).map(c => c.get_first_child() === bar.clock ? 'clock'
                    : c.get_first_child() === bar.workspaces ? 'workspaces' : 'other'),
            labels: () => bar.row?.actor.get_children() ?? [],
        }));
    }

    peeking(monitor) {
        return this._peeks?.has(monitor) ?? false;
    }

    // ---- The bars ----

    _eachBar(f) {
        for (const bar of this._bars?.values() ?? [])
            f(bar);
    }

    // Many things change at once (screens plugged in, the tiling starting): the bars are made once, before the next
    // frame, after all of them
    _queueSync() {
        if (this._later || !this._bars)
            return;
        this._later = global.compositor.get_laters().add(Meta.LaterType.BEFORE_REDRAW, () => {
            this._later = 0;
            this._sync();
            return GLib.SOURCE_REMOVE;
        });
    }

    _sync() {
        if (!this._bars)
            return;
        const layout = Main.layoutManager;
        const wanted = new Map();
        if (this._settings.get_boolean('every-screen') && layout.monitors.length > 1) {
            const names = connectors();
            layout.monitors.forEach((monitor, index) => {
                if (index !== layout.primaryIndex)
                    wanted.set(names[index] ?? `monitor-${index}`, index);
            });
        }
        let changed = false;
        const gone = [];
        for (const [connector, bar] of [...this._bars]) {
            if (!wanted.has(connector)) {
                this._endPeek(bar.monitor);
                this._bars.delete(connector);
                gone.push(bar);
            }
        }
        // The others let go of a bar before it goes
        if (gone.length) {
            this._told();
            gone.forEach(bar => bar.destroy());
        }
        for (const [connector, index] of wanted) {
            let bar = this._bars.get(connector);
            if (!bar) {
                bar = new Bar(this, index, connector);
                this._bars.set(connector, bar);
                changed = true;
            } else if (bar.monitor !== index) {
                this._endPeek(bar.monitor);
                bar.place(index);
                changed = true;
            } else {
                bar.place(index);
            }
        }
        this._setClock(this._bars.size > 0);
        this._hookScreens(this._screens);
        this._rows();
        if (changed)
            this._told();
        layout._updateVisibility();
    }

    // The workspaces are shown when each screen has a workspace of its own (the tiling on several screens), or when
    // GNOME's workspaces span every screen; otherwise the other screens' windows are on every workspace, and their
    // bars hold the clock alone
    _rows() {
        const separate = this._screens?.separate ?? false;
        const everywhere = !this._ext.modules?.get('tiling')?.running && !this._mutter.get_boolean(ONLY_PRIMARY);
        this._eachBar(bar => bar.setRow(separate || everywhere));
        this.workspacesModule?.refreshRows?.();
    }

    // The others hear of the bars once they changed: the style marks their buttons, the glass gives them their panes,
    // the tiling puts its line under them
    _told() {
        const modules = this._ext.modules;
        modules?.get('style')?.refresh?.();
        modules?.get('glass')?.surfacesChanged?.();
        modules?.get('tiling')?.topBar?.positionsChanged?.();
    }

    // The tiling's screens say when a screen shows another workspace
    _hookScreens(screens) {
        if (screens === this._hooked)
            return;
        if (this._hooked && this._hookedId)
            this._hooked.disconnect(this._hookedId);
        this._hooked = screens;
        this._hookedId = screens?.connect?.('changed', () => {
            this._rows();
            Main.layoutManager._updateVisibility();
        }) ?? 0;
    }

    // One wall clock for every bar, ticking only while there is one; it says the time as GNOME's own clock does
    _setClock(on) {
        if (on && !this._clock) {
            this._clock = new GnomeDesktop.WallClock();
            this._clock.connectObject('notify::clock', () => this._tick(), this);
        } else if (!on && this._clock) {
            this._clock.disconnectObject(this);
            this._clock.run_dispose();
            this._clock = null;
        }
        this._tick();
    }

    _tick() {
        const text = this._clock?.clock ?? '';
        this._eachBar(bar => {
            if (bar.time.text !== text)
                bar.time.text = text;
        });
    }

    // ---- Hidden and peeking ----

    // GNOME hides what gives way to fullscreen windows; a bar peeking stays, and a secondary bar is hidden on the
    // workspaces where the tiling hides the top bar, giving its height back to the windows
    _wrapVisibility() {
        const layout = Main.layoutManager;
        const original = layout._updateActorVisibility;
        if (typeof original !== 'function')
            return;
        this._ownVisibility = Object.prototype.hasOwnProperty.call(layout, '_updateActorVisibility') ? original : null;
        const self = this;
        layout._updateActorVisibility = function (actorData) {
            original.call(this, actorData);
            self._visibility(actorData);
        };
        this._wrapped = true;
    }

    _unwrapVisibility() {
        if (!this._wrapped)
            return;
        if (this._ownVisibility)
            Main.layoutManager._updateActorVisibility = this._ownVisibility;
        else
            delete Main.layoutManager._updateActorVisibility;
        this._wrapped = false;
        this._ownVisibility = null;
    }

    _visibility(actorData) {
        if (!this._bars)
            return;
        const actor = actorData.actor;
        const layout = Main.layoutManager;
        // Hidden until touched: never a strut, and hidden while the windows show (the overview still has it)
        const tucked = this._autohide && global.window_group.visible;
        if (actor === layout.panelBox) {
            this._struts(actorData, !this._autohide);
            if (this._peeks.has(layout.primaryIndex))
                actor.visible = true;
            else if (tucked)
                actor.visible = false;
            return;
        }
        const bar = [...this._bars.values()].find(b => b.box === actor);
        if (!bar)
            return;
        const topBar = this._ext.modules?.get('tiling')?.topBar;
        const hidden = !!topBar && !!this._screens && topBar.hiddenOn(this._screens.shownOn(bar.monitor));
        const room = !(hidden || this._autohide);
        if (actorData.affectsStruts !== room) {
            actorData.affectsStruts = room;
            layout._queueUpdateRegions();
        }
        if (this._peeks.has(bar.monitor))
            actor.visible = true;
        else if (hidden || tucked)
            actor.visible = false;
    }

    // The main bar's strut, taken away while it hides and given back after
    _struts(actorData, keep) {
        if (!keep && actorData.affectsStruts) {
            actorData.affectsStruts = false;
            this._strutless.add(actorData);
            Main.layoutManager._queueUpdateRegions();
        } else if (keep && this._strutless.has(actorData)) {
            this._strutless.delete(actorData);
            actorData.affectsStruts = true;
            Main.layoutManager._queueUpdateRegions();
        }
    }

    // ---- Size, edge, hidden until touched ----

    _syncSize() {
        const size = this._settings?.get_string('size');
        const group = Main.layoutManager.uiGroup;
        for (const [name, cls] of Object.entries(SIZES)) {
            if (name === size)
                group.add_style_class_name(cls);
            else
                group.remove_style_class_name(cls);
        }
    }

    // GNOME places the main bar's box at the top of the main screen whenever the screens change: at the bottom, it is
    // moved down after
    _wrapBoxes() {
        const layout = Main.layoutManager;
        const original = layout._updateBoxes;
        if (typeof original !== 'function')
            return;
        this._ownBoxes = Object.prototype.hasOwnProperty.call(layout, '_updateBoxes') ? original : null;
        const self = this;
        layout._updateBoxes = function () {
            original.call(this);
            self._placeMain();
        };
        this._boxesWrapped = true;
    }

    _unwrapBoxes() {
        if (!this._boxesWrapped)
            return;
        if (this._ownBoxes)
            Main.layoutManager._updateBoxes = this._ownBoxes;
        else
            delete Main.layoutManager._updateBoxes;
        this._boxesWrapped = false;
        this._ownBoxes = null;
    }

    _placeMain() {
        const layout = Main.layoutManager;
        const monitor = layout.primaryMonitor;
        if (!this._bottom || !monitor)
            return;
        layout.panelBox.set_position(monitor.x, monitor.y + monitor.height - layout.panelBox.height);
    }

    _syncPosition() {
        if (!this._settings)
            return;
        const bottom = this._settings.get_string('position') === 'bottom';
        const layout = Main.layoutManager;
        this._autohide = this._settings.get_boolean('autohide');
        if (bottom !== !!this._bottom) {
            this._bottom = bottom;
            if (bottom) {
                layout.uiGroup.add_style_class_name(BOTTOM);
                layout.panelBox.connectObject('notify::height', () => {
                    this._placeMain();
                    this._syncSpacers();
                }, this);
            } else {
                layout.uiGroup.remove_style_class_name(BOTTOM);
                layout.panelBox.disconnectObject(this);
            }
        }
        layout._updateBoxes();
        this._eachBar(bar => bar.place(bar.monitor));
        this._syncMenus();
        this._rebuildBarriers();
        layout._updateVisibility();
        layout._queueUpdateRegions();
        this._ext.modules?.get('dock')?.docks?.forEach?.(site => site.place());
        this._syncSpacers();
        this._ext.modules?.get('tiling')?.topBar?.positionsChanged?.();
    }

    // The menus of the bar's buttons open away from the edge it sits on
    _syncMenus() {
        if (!this._turned)
            return;
        for (const button of Object.values(Main.panel.statusArea)) {
            const pointer = button?.menu?._boxPointer;
            if (!pointer?.updateArrowSide)
                continue;
            const now = pointer._userArrowSide;
            if (this._bottom && now === St.Side.TOP) {
                pointer.updateArrowSide(St.Side.BOTTOM);
                this._turned.add(pointer);
            } else if (!this._bottom && this._turned.has(pointer) && now === St.Side.BOTTOM) {
                pointer.updateArrowSide(St.Side.TOP);
            }
        }
        if (!this._bottom)
            this._turned.clear();
    }

    // A dock at the bottom of a screen whose bar is at the bottom too sits above the bar; GNOME keeps no room for an
    // actor that touches neither edge, so a transparent strip at the edge keeps the room of both
    _syncSpacers() {
        if (!this._spacers)
            return;
        const layout = Main.layoutManager;
        const wanted = new Map();
        if (this._bottom && !this._autohide) {
            for (const site of this._ext.modules?.get('dock')?.docks ?? []) {
                if (site.edgeName !== 'bottom' || !site.reserves || !site.thickness)
                    continue;
                const monitor = layout.monitors[site.monitorIndex];
                const inset = this.bottomInset(site.monitorIndex);
                if (!monitor || !inset)
                    continue;
                const height = Math.round(inset + site.thickness);
                wanted.set(site.monitorIndex, {x: monitor.x, y: monitor.y + monitor.height - height, width: monitor.width, height});
            }
        }
        for (const [index, spacer] of [...this._spacers]) {
            if (!wanted.has(index)) {
                this._spacers.delete(index);
                spacer.destroy();
            }
        }
        for (const [index, rect] of wanted) {
            let spacer = this._spacers.get(index);
            if (!spacer) {
                spacer = new St.Widget({name: 'amethystora-bars-room', reactive: false, opacity: 0});
                Main.layoutManager.addChrome(spacer, {affectsStruts: true});
                this._spacers.set(index, spacer);
            }
            if (spacer.x !== rect.x || spacer.y !== rect.y || spacer.width !== rect.width || spacer.height !== rect.height) {
                spacer.set_position(rect.x, rect.y);
                spacer.set_size(rect.width, rect.height);
                layout._queueUpdateRegions();
            }
        }
    }

    // The bar of a screen and the panel in it that slides
    _barOf(monitor) {
        const layout = Main.layoutManager;
        if (monitor === layout.primaryIndex)
            return {box: layout.panelBox, panel: Main.panel, main: true};
        for (const bar of this._bars.values()) {
            if (bar.monitor === monitor)
                return {box: bar.box, panel: bar.panel, main: false};
        }
        return null;
    }

    // A barrier along the top edge of each screen a window is fullscreen on, while the peek is on
    _syncPeek() {
        if (!this._barriers)
            return;
        const on = this._settings.get_boolean('peek');
        const layout = Main.layoutManager;
        const wanted = new Set(on ? layout.monitors.filter(m => m.inFullscreen).map(m => m.index) : []);
        // Hidden until touched: the edge of every screen with a bar
        if (this._autohide) {
            if (layout.primaryMonitor)
                wanted.add(layout.primaryIndex);
            this._eachBar(bar => wanted.add(bar.monitor));
        }
        for (const index of [...this._barriers.keys()]) {
            if (!wanted.has(index))
                this._dropBarrier(index);
        }
        for (const index of [...this._peeks.keys()]) {
            if (!wanted.has(index))
                this._endPeek(index);
        }
        for (const index of wanted) {
            if (!this._barriers.has(index))
                this._makeBarrier(index);
        }
    }

    _rebuildBarriers() {
        if (!this._barriers)
            return;
        for (const index of [...this._peeks.keys()])
            this._endPeek(index);
        for (const index of [...this._barriers.keys()])
            this._dropBarrier(index);
        this._syncPeek();
    }

    // Along the parts of the top edge with no screen above them: the way up to a screen above stays open
    _makeBarrier(index) {
        const layout = Main.layoutManager;
        const monitor = layout.monitors[index];
        if (!monitor)
            return;
        const bottom = !!this._bottom;
        const spans = bottom ? freeBottomEdge(monitor, layout.monitors) : freeTopEdge(monitor, layout.monitors);
        if (!spans.length)
            return;
        const pressure = new Layout.PressureBarrier(PRESSURE, PRESSURE_MS, Shell.ActionMode.NORMAL);
        const y = bottom ? monitor.y + monitor.height : monitor.y;
        // A barrier's ends are pixels it covers: each stops at the last pixel of its span
        const barriers = spans.map(([x1, x2]) => new Meta.Barrier({
            backend: global.backend,
            x1, x2: x2 - 1, y1: y, y2: y,
            directions: bottom ? Meta.BarrierDirection.NEGATIVE_Y : Meta.BarrierDirection.POSITIVE_Y,
        }));
        barriers.forEach(barrier => pressure.addBarrier(barrier));
        pressure.connect('trigger', () => this._peek(index));
        this._barriers.set(index, {barriers, pressure});
    }

    _dropBarrier(index) {
        const entry = this._barriers.get(index);
        if (!entry)
            return;
        this._barriers.delete(index);
        entry.pressure.disconnectAll?.();
        entry.pressure.destroy();
        entry.barriers.forEach(barrier => barrier.destroy());
    }

    // The bar comes down over the fullscreen window, from where it is if it was on its way back up
    _peek(index) {
        // A window handed over (R7-64) is all there is: the bar, with the notifications and the power menu, stays up
        if (this._ext.modules?.get('windows')?.handOver?.active)
            return;
        const bar = this._barOf(index);
        if (!bar)
            return;
        let peek = this._peeks.get(index);
        // A bar the tiling already shows (its fullscreen window scrolled off the strip) needs no peek
        if (!peek && bar.box.visible)
            return;
        if (!peek) {
            peek = {
                ...bar,
                timer: 0,
                menu: null,
                clip: bar.box.clip_to_allocation,
                reactive: bar.box.reactive,
                hover: bar.box.track_hover,
                going: false,
                // Whether Clutter has told the bar the pointer is on it, and how often it was looked for since
                hoverSeen: false,
                rechecks: 0,
            };
            this._peeks.set(index, peek);
            // The pointer on the bar keeps it; away from it, the bar goes after a moment. Its whole box takes the
            // pointer while it peeks, margins included, so that the edge the pointer was pushed against is on it.
            bar.box.reactive = true;
            bar.box.track_hover = true;
            bar.box.connectObject('notify::hover', () => this._hoverChanged(index), peek);
            // GNOME shows it again now that it is peeking (_visibility)
            Main.layoutManager._updateVisibility();
            bar.panel.translation_y = this._away(bar.box);
        }
        peek.going = false;
        this._stopTimer(peek);
        bar.box.clip_to_allocation = true;
        bar.panel.remove_transition('translation-y');
        easeCurved(bar.panel, {translation_y: 0}, SLIDE_MS, () => {
            if (this._peeks?.get(index) !== peek || peek.going)
                return;
            bar.box.clip_to_allocation = peek.clip;
            // Where it now takes clicks, at its place
            Main.layoutManager._queueUpdateRegions();
            if (!peek.box.hover)
                this._armAway(index);
        });
    }

    // Where a peeking bar slides in from: above its screen, or below it at the bottom
    _away(box) {
        return this._bottom ? box.height : -box.height;
    }

    _inside(peek, x, y) {
        const [bx, by] = peek.box.get_transformed_position();
        const [bw, bh] = peek.box.get_transformed_size();
        return x >= bx && x < bx + bw && y >= by && y < by + bh;
    }

    _hoverChanged(index) {
        const peek = this._peeks?.get(index);
        if (!peek || peek.going)
            return;
        if (peek.box.hover) {
            peek.hoverSeen = true;
            this._stopTimer(peek);
        } else {
            this._armAway(index);
        }
    }

    _stopTimer(peek) {
        if (peek.timer)
            GLib.source_remove(peek.timer);
        peek.timer = 0;
    }

    _armAway(index) {
        const peek = this._peeks?.get(index);
        if (!peek || peek.going || peek.menu)
            return;
        this._stopTimer(peek);
        peek.timer = GLib.timeout_add(GLib.PRIORITY_DEFAULT, AWAY_MS, () => {
            peek.timer = 0;
            this._maybeGo(index);
            return GLib.SOURCE_REMOVE;
        });
    }

    // Away long enough: unless a menu of the bar is open (then it waits for it to close) or the pointer is on the bar
    // after all (the bar may have come under a pointer that has not moved since, which Clutter has not told it of yet)
    _maybeGo(index) {
        const peek = this._peeks?.get(index);
        if (!peek || peek.going)
            return;
        const menu = peek.main ? Main.panel.menuManager.activeMenu : null;
        if (menu?.isOpen) {
            peek.menu = menu;
            menu.connectObject('open-state-changed', (m, open) => {
                if (open)
                    return;
                menu.disconnectObject(this);
                peek.menu = null;
                this._armAway(index);
            }, this);
            return;
        }
        // On the bar after all: it stays. Once Clutter has told the bar the pointer is on it, leaving says so (hover)
        // and starts the moment again. Until then (the bar came down under a pointer resting at the edge) a pointer
        // flicked straight off it would say nothing, so it is looked for again at the same pace, only while it rests
        // on the bar during a peek; a few more times after hover was seen, for the moment Clutter takes to say so.
        const [x, y] = global.get_pointer();
        if (peek.box.hover)
            return;
        if (this._inside(peek, x, y)) {
            if (!peek.hoverSeen || ++peek.rechecks <= RECHECKS)
                this._armAway(index);
            return;
        }
        this._go(index);
    }

    // Back up, then GNOME hides it again
    _go(index) {
        const peek = this._peeks.get(index);
        peek.going = true;
        this._stopTimer(peek);
        peek.box.clip_to_allocation = true;
        easeCurved(peek.panel, {translation_y: this._away(peek.box)}, SLIDE_MS, () => {
            if (this._peeks?.get(index) === peek && peek.going)
                this._endPeek(index);
        });
    }

    // The peek over, at once: the bar back in its place and GNOME's rule for fullscreen windows again
    _endPeek(index) {
        const peek = this._peeks?.get(index);
        if (!peek)
            return;
        this._peeks.delete(index);
        this._stopTimer(peek);
        peek.menu?.disconnectObject(this);
        peek.box.disconnectObject(peek);
        peek.panel.remove_transition('translation-y');
        peek.panel.translation_y = 0;
        peek.box.clip_to_allocation = peek.clip;
        peek.box.reactive = peek.reactive;
        peek.box.track_hover = peek.hover;
        Main.layoutManager._updateVisibility();
        Main.layoutManager._queueUpdateRegions();
    }
}
