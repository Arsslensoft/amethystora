// The workspaces as a row of labels in the top bar, after the logo menu. The one you are on is filled with the
// theme's accent, one with windows on it is in the theme's text colour, and an empty one is fainter. Click a label to
// go there: the one you are on shows or hides the overview instead, and an empty one opens the overview, ready to
// search or start an app. Scrolling anywhere on the top bar steps to the previous or next workspace. Drag a label
// along the row to move its workspace, windows and all, or drop a window from the overview on a label to send the
// window there: while a window is dragged over a label, the label lights up and a note under the bar says where the
// window would go, and a small count shows on the label once it has landed. A label shows its workspace's number and,
// after it, the workspace's name: one you gave it, or else, in a lighter slanted type, the name of the app with most
// windows on it, worked out each time and never stored (ame desktop space-names turns that off). A right click, or a
// finger held on a label, opens a field to rename the workspace you are on. Two keys
// move the workspace you are on one place to the left or right. There are always as many workspaces as GNOME is set
// to keep (six), so that Super+1 to Super+6 reach every one: nothing here adds or removes one. The workspaces, their
// names and the keys that switch them are GNOME's own. The same menu saves the workspace's apps and their places as
// a set, and opens a saved set again (workspaces/sets.js, ame desktop sets).

import Atk from 'gi://Atk';
import Clutter from 'gi://Clutter';
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import GObject from 'gi://GObject';
import Meta from 'gi://Meta';
import Shell from 'gi://Shell';
import St from 'gi://St';

import {gettext as _} from 'resource:///org/gnome/shell/extensions/extension.js';
import {adjustAnimationTime} from 'resource:///org/gnome/shell/misc/animationUtils.js';
import * as BoxPointer from 'resource:///org/gnome/shell/ui/boxpointer.js';
import * as DND from 'resource:///org/gnome/shell/ui/dnd.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import * as PanelMenu from 'resource:///org/gnome/shell/ui/panelMenu.js';
import * as PopupMenu from 'resource:///org/gnome/shell/ui/popupMenu.js';

import {ACCENTS, SCHEMA as THEME_SCHEMA, accentName, workspaceAccent} from './theme/accents.js';
import {Sets} from './workspaces/sets.js';
import {SetsMenu} from './workspaces/setsmenu.js';

const ROLE = 'amethystora-workspaces';
const SCHEMA = 'org.gnome.shell.extensions.amethystora-ui.workspaces';
const KEYS = ['move-workspace-left', 'move-workspace-right'];
// GNOME's own list of names, by workspace position
const PREFS = 'org.gnome.desktop.wm.preferences';
const NAMES = 'workspace-names';
// A press held longer than this before its release is no click: it may have been the start of a drag
const CLICK_MS = 300;
// How long a finger rests on a label before the menu opens
const LONG_PRESS_MS = 500;
// At most one workspace per this long when scrolling, so that a fast spin of the wheel does not race to the end
const SCROLL_PAUSE_MS = 200;
// How long a gap in the row takes to open or close while a label is dragged along it
const GAP_MS = 500;
const STATES = ['active', 'busy', 'empty'];
// A window dragged over a label: how much larger the label grows, and how long it takes, before the slow-down factor
const HIT_SCALE = 1.12;
const HIT_MS = 250;
// The count on a label a window was dropped on: how long it stays, and how long it takes to fade
const BADGE_MS = 1500;
const BADGE_FADE_MS = 250;
// How far below the top bar the note says where a window would go
const HINT_GAP = 6;

// The app a workspace is about: the one with most of its own windows there (minimized ones count), a tie going to the
// app whose oldest window there is oldest, so that the name does not change as you click between two windows. Windows
// on every workspace, kept out of the taskbar or fixed to another window, and apps known only by a window's title,
// say nothing.
function mainApp(workspace) {
    const tracker = Shell.WindowTracker.get_default();
    const counts = new Map();
    for (const window of workspace.list_windows()) {
        if (window.get_window_type() !== Meta.WindowType.NORMAL || window.skip_taskbar ||
            window.is_on_all_workspaces() || window.is_attached_dialog())
            continue;
        const app = tracker.get_window_app(window);
        if (!app || app.is_window_backed())
            continue;
        const count = counts.get(app) ?? {n: 0, oldest: Infinity};
        count.n++;
        count.oldest = Math.min(count.oldest, window.get_stable_sequence());
        counts.set(app, count);
    }
    let best = null;
    for (const [app, count] of counts) {
        if (!best || count.n > best.n || (count.n === best.n && count.oldest < best.oldest))
            best = {app, ...count};
    }
    return best?.app ?? null;
}

// A workspace counts as having windows when one of them lives on it alone: a window on every workspace (pinned, or
// on another monitor while only the main one has workspaces) says nothing about this one
function hasWindows(workspace) {
    return workspace.list_windows().some(w => !w.is_on_all_workspaces());
}

// The window to focus on arriving: the one last used there, as Alt+Tab orders them, where a dialog fixed to its
// window stands for that window and windows kept out of the taskbar or on every workspace are passed over
function recentWindow(workspace) {
    for (let window of global.display.get_tab_list(Meta.TabList.NORMAL_ALL, workspace)) {
        if (window.is_attached_dialog())
            window = window.get_transient_for() ?? window;
        if (!window.skip_taskbar && !window.is_on_all_workspaces())
            return window;
    }
    return null;
}

function isRtl(actor) {
    return actor.get_text_direction() === Clutter.TextDirection.RTL;
}

// One workspace's label: its number and its name. It is its own drag source, and takes a window dropped on it. A
// plain one (another screen's bar) only takes a click and a window: no drag, no menu.
const WorkspaceLabel = GObject.registerClass(
class WorkspaceLabel extends St.BoxLayout {
    _init(row, plain = false) {
        super._init({
            style_class: 'amethystora-workspaces-label',
            reactive: true,
            y_align: Clutter.ActorAlign.CENTER,
            accessible_role: Atk.Role.PUSH_BUTTON,
        });
        // Names are text, whoever wrote them
        this._number = new St.Label({style_class: 'amethystora-workspaces-number', y_align: Clutter.ActorAlign.CENTER});
        this._name = new St.Label({style_class: 'amethystora-workspaces-name', y_align: Clutter.ActorAlign.CENTER});
        this._number.clutter_text.use_markup = false;
        this._name.clutter_text.use_markup = false;
        this._name.hide();
        this.add_child(this._number);
        this.add_child(this._name);
        this._row = row;
        this._delegate = this;
        this.badge = null;
        this.index = -1;

        // A click goes on the release, and only when it came soon after its press, so that a press that turns
        // into a drag never switches
        const click = new Clutter.ClickGesture({required_button: Clutter.BUTTON_PRIMARY});
        click.connect('notify::pressed', gesture => {
            if (gesture.pressed)
                this._pressedAt = GLib.get_monotonic_time();
        });
        click.connect('recognize', () => {
            if (GLib.get_monotonic_time() - this._pressedAt <= CLICK_MS * 1000)
                row.clicked(this.index);
        });
        this.add_action(click);
        if (plain)
            return;

        // A second right click closes the menu again
        const menu = new Clutter.ClickGesture({required_button: Clutter.BUTTON_SECONDARY, recognize_on_press: true});
        menu.connect('recognize', () => row.toggleMenu(this.index));
        this.add_action(menu);

        // A finger only: a mouse button held still is the start of a click or a drag
        const hold = new Clutter.LongPressGesture({long_press_duration_ms: LONG_PRESS_MS});
        hold.connect('should-handle-sequence', (gesture, event) => event.type() === Clutter.EventType.TOUCH_BEGIN);
        hold.connect('recognize', () => row.openMenu(true, this.index));
        this.add_action(hold);

        this._draggable = DND.makeDraggable(this);
        this._draggable.connect('drag-begin', () => row.dragBegan(this));
        this._draggable.connect('drag-cancelled', () => row.dragCancelled());
        this._draggable.connect('drag-end', () => row.dragEnded(this));
    }

    // What the label shows, as one line of text ("3 Files", or "3")
    get labelText() {
        const number = this._number.text;
        return this._name.visible ? `${number} ${this._name.text}` : number;
    }

    // title: the workspace's name or the empty string; auto: whether it is its main app's rather than one given
    update(index, title, auto, state) {
        this.index = index;
        const number = `${index + 1}`;
        if (this._number.text !== number)
            this._number.text = number;
        if (this._name.text !== title)
            this._name.text = title;
        this._name.visible = title !== '';
        if (auto && title !== '')
            this._name.add_style_class_name('amethystora-workspaces-auto');
        else
            this._name.remove_style_class_name('amethystora-workspaces-auto');
        if (this.accessible_description !== title)
            this.accessible_description = title;
        for (const name of STATES) {
            if (name === state)
                this.add_style_class_name(`amethystora-workspaces-${name}`);
            else
                this.remove_style_class_name(`amethystora-workspaces-${name}`);
        }
        const accessible = _('Workspace %d').format(index + 1);
        if (this.accessible_name !== accessible)
            this.accessible_name = accessible;
    }

    // Another label passes on to the row, which makes room for it; anything but a window is refused there
    handleDragOver(source) {
        return source?.metaWindow ? DND.DragMotionResult.MOVE_DROP : DND.DragMotionResult.CONTINUE;
    }

    acceptDrop(source) {
        if (!source?.metaWindow)
            return false;
        source.metaWindow.change_workspace_by_index(this.index, false);
        this._row.landed(this);
        return true;
    }
});

// The panel button that holds the row. It has no click of its own and no highlight under the pointer: only its
// labels answer. The row itself takes a label dropped on it (the button's own drop handler is the place-holder menu
// GNOME gives a button without one).
const WorkspaceRow = GObject.registerClass(
class WorkspaceRow extends PanelMenu.Button {
    _init(module) {
        super._init(0.5, _('Workspaces'), true);
        this.track_hover = false;
        this.add_style_class_name('amethystora-workspaces-button');
        this.box = new St.BoxLayout({style_class: 'amethystora-workspaces-row', y_align: Clutter.ActorAlign.CENTER});
        this.box._delegate = {
            handleDragOver: (source, actor) => module.dragOver(source, actor),
            acceptDrop: (source, actor) => module.drop(source, actor),
        };
        this.add_child(this.box);
    }
});

// The workspaces in another screen's top bar (modules/bars.js): the same labels, lit for the workspace that screen
// shows. A click does what it does on the main bar, on that screen; a window dropped on a label moves to that workspace.
// Labels here start no drag and open no menu, and the wheel does nothing: the main bar keeps those.
class OtherRow {
    constructor(module, monitor) {
        this._module = module;
        this.monitor = monitor;
        this.actor = new St.BoxLayout({style_class: 'amethystora-workspaces-row', y_align: Clutter.ActorAlign.CENTER});
        this._labels = [];
        this._active = -1;
    }

    activeIndex() {
        return this._active;
    }

    clicked(index) {
        this._module.clicked(index, this.monitor);
    }

    landed(label) {
        this._module.landed(label);
    }

    // Brought up to date with what the main row worked out, `active` being the workspace this screen shows
    fill(described, active) {
        this._active = active;
        while (this._labels.length < described.length) {
            const label = new WorkspaceLabel(this, true);
            label.connect('destroy', () => {
                this._module._dropBadge(label);
                const at = this._labels.indexOf(label);
                if (at >= 0)
                    this._labels.splice(at, 1);
            });
            this._labels.push(label);
            this.actor.add_child(label);
        }
        while (this._labels.length > described.length)
            this._labels.at(-1).destroy();
        this._labels.forEach((label, index) => {
            const {title, auto, busy} = described[index];
            label.update(index, title, auto, index === active ? 'active' : busy ? 'busy' : 'empty');
        });
    }

    destroy() {
        this._module.forgetRow(this);
        this.actor.destroy();
    }
}

export class Workspaces {
    constructor(ext) {
        this._ext = ext;
    }

    enable() {
        this._labels = [];
        this._others = new Set();
        this._watched = new Set();
        this._prefs = new Gio.Settings({schema_id: PREFS});

        this._row = new WorkspaceRow(this);
        this._row.connect('destroy', () => {
            this._row = null;
        });
        Main.panel.addToStatusArea(ROLE, this._row, 1, 'left');
        this._buildMenu();

        const queue = () => this._queue();
        global.workspace_manager.connectObject(
            'notify::n-workspaces', queue,
            'workspace-added', queue,
            'workspace-removed', queue,
            'workspaces-reordered', () => this._reordered(),
            'active-workspace-changed', queue, this);
        Shell.WindowTracker.get_default().connectObject('tracked-windows-changed', queue, this);
        this._prefs.connectObject(`changed::${NAMES}`, queue, this);

        // Scrolling anywhere on the bar that nothing under the pointer took for itself
        Main.panel.connectObject('scroll-event', (actor, event) => this._scrolled(event), this);

        this._settings = this._ext.getSettings(SCHEMA);
        this._settings.connectObject('changed::auto-names', queue, this);
        // While each workspace has an accent of its own, the menu chooses the one of the workspace it was opened on
        this._themeSettings = this._ext.getSettings(THEME_SCHEMA);
        this._themeSettings.connectObject('changed::accent-per-workspace', () => this._buildAccents(), this);
        this._buildAccents();
        // Saved sets, while they are on
        this._settings.connectObject('changed::sets', () => this._syncSets(), this);
        this._syncSets();
        // A window dragged in the overview: the label under it lights up
        Main.overview.connectObject(
            'window-drag-begin', () => this._windowDragBegan(),
            'window-drag-end', () => this._windowDragEnded(),
            'window-drag-cancelled', () => this._windowDragEnded(),
            this);
        const modes = Shell.ActionMode.NORMAL | Shell.ActionMode.OVERVIEW;
        Main.wm.addKeybinding(KEYS[0], this._settings, Meta.KeyBindingFlags.NONE, modes, () => this._shift(-1));
        Main.wm.addKeybinding(KEYS[1], this._settings, Meta.KeyBindingFlags.NONE, modes, () => this._shift(1));
        this._keys = true;

        this._refresh();
    }

    disable() {
        if (this._keys) {
            for (const key of KEYS)
                Main.wm.removeKeybinding(key);
        }
        this._keys = false;
        this._settings?.disconnectObject(this);
        this._themeSettings?.disconnectObject(this);
        // Disposed now rather than by the collector (dock.js says why)
        this._themeSettings?.run_dispose();
        this._themeSettings = null;
        this._accentMenu?.destroy();
        this._accentMenu = null;
        this._settings?.run_dispose();
        this._settings = null;

        Main.overview.disconnectObject(this);
        this._windowDragEnded();
        for (const label of this._labels ?? [])
            this._dropBadge(label);

        Main.panel.disconnectObject(this);
        global.workspace_manager.disconnectObject(this);
        Shell.WindowTracker.get_default().disconnectObject(this);
        this._prefs?.disconnectObject(this);
        for (const workspace of this._watched ?? [])
            workspace.disconnectObject(this);
        this._watched = null;

        if (this._later)
            global.compositor.get_laters().remove(this._later);
        this._later = 0;
        if (this._scrollPause)
            GLib.source_remove(this._scrollPause);
        this._scrollPause = 0;

        // A label being dragged lives outside the row until the drag ends: taking it away ends the drag
        const dragged = this._drag?.label;
        this._drag = null;
        this._dropped = null;
        dragged?.destroy();

        this._stopSets();
        this._menu?.destroy();
        this._menu = null;
        for (const row of [...this._others ?? []])
            row.destroy();
        this._others = null;
        this._row?.destroy();
        this._row = null;
        this._labels = null;
        this._order = null;
        this._prefs?.run_dispose();
        this._prefs = null;
    }

    // Everything that can change a label lands here, and the row is brought up to date once, before the next frame
    _queue() {
        if (this._later)
            return;
        this._later = global.compositor.get_laters().add(Meta.LaterType.BEFORE_REDRAW, () => {
            this._later = 0;
            this._refresh();
            return GLib.SOURCE_REMOVE;
        });
    }

    // The row has a label per workspace, the label at a place standing for the workspace at that place. Only what
    // changed is touched, so a refresh that finds the row as it was draws nothing.
    _refresh() {
        if (!this._row || this._drag)
            return;
        if (this._dropped) {
            // Back from a drag that went nowhere: put back in the row at its end, so it is made again in its place
            const label = this._dropped;
            this._dropped = null;
            label.destroy();
        }

        const manager = global.workspace_manager;
        const count = manager.n_workspaces;
        while (this._labels.length < count) {
            const label = new WorkspaceLabel(this);
            label.connect('destroy', () => {
                this._dropBadge(label);
                const at = this._labels?.indexOf(label) ?? -1;
                if (at >= 0)
                    this._labels.splice(at, 1);
            });
            this._labels.push(label);
            this._row.box.add_child(label);
        }
        while (this._labels.length > count)
            this._labels.at(-1).destroy();

        // With a workspace per screen, GNOME's own is the screen with the keyboard's: this bar lights what its screen shows
        const active = this._shownOn(Main.layoutManager.primaryIndex)?.index() ?? manager.get_active_workspace_index();
        const names = this._prefs.get_strv(NAMES);
        const auto = this._settings?.get_boolean('auto-names') ?? false;
        this._order = this._workspaces();
        // What each label says, worked out once for this row and the other screens' rows
        const described = this._order.map((workspace, index) => {
            const own = names[index] ?? '';
            const app = !own && auto ? mainApp(workspace) : null;
            return {title: own || (app?.get_name() ?? ''), auto: !own && !!app, busy: hasWindows(workspace)};
        });
        this._labels.forEach((label, index) => {
            const {title, auto: named, busy} = described[index];
            label.update(index, title, named, index === active ? 'active' : busy ? 'busy' : 'empty');
        });
        for (const row of this._others)
            row.fill(described, this._shownOn(row.monitor)?.index() ?? active);

        // A window opened, closed or moved onto or off any workspace
        const workspaces = new Set(this._order);
        for (const workspace of this._watched) {
            if (!workspaces.has(workspace))
                workspace.disconnectObject(this);
        }
        for (const workspace of workspaces) {
            if (!this._watched.has(workspace)) {
                workspace.connectObject(
                    'window-added', () => this._queue(),
                    'window-removed', () => this._queue(), this);
            }
        }
        this._watched = workspaces;
    }

    _workspaces() {
        const manager = global.workspace_manager;
        const workspaces = [];
        for (let index = 0; index < manager.n_workspaces; index++)
            workspaces.push(manager.get_workspace_by_index(index));
        return workspaces;
    }

    // A workspace that something else moved keeps its name as well: the names follow the workspaces from the order
    // they were last seen in. A move of the row's own has written them already.
    _reordered() {
        const now = this._workspaces();
        if (!this._moving && this._order) {
            const names = this._namesFor(this._order.length);
            this._writeNames(now.map(workspace => names[this._order.indexOf(workspace)] ?? ''));
        }
        this._order = now;
        this._queue();
    }

    // Every click on the one you are on shows or hides the overview, even one that comes while it is still sliding. With
    // a workspace per screen the click is about this bar's screen: the workspace comes there, as the wheel's does.
    // A click on a label of the bar of screen `monitor`: the workspace is shown on that screen, swapping with the screen
    // that showed it; with GNOME's workspaces spanning every screen, every screen goes there
    clicked(index, monitor = Main.layoutManager.primaryIndex) {
        const manager = global.workspace_manager;
        const shown = this._shownOn(monitor);
        if (index === (shown?.index() ?? manager.get_active_workspace_index())) {
            Main.overview.toggle();
            return;
        }
        const workspace = manager.get_workspace_by_index(index);
        if (!workspace)
            return;
        const empty = !hasWindows(workspace) && !Main.overview.visible;
        if (shown)
            this._ext.modules.get('tiling').engine.screens.bring(workspace, monitor);
        else if (empty)
            workspace.activate(global.get_current_time());
        else
            this._goTo(workspace);
        if (empty)
            Main.overview.show();
    }

    // ---- The rows of the other screens' bars (modules/bars.js) ----

    // A row of labels for the bar of screen `monitor`: {actor, monitor, activeIndex(), destroy()}
    rowFor(monitor) {
        const row = new OtherRow(this, monitor);
        this._others?.add(row);
        this._queue();
        return row;
    }

    forgetRow(row) {
        this._others?.delete(row);
    }

    // The screens changed what they show
    refreshRows() {
        if (this._others?.size)
            this._queue();
    }

    // The workspace a screen shows: its own with the tiling on several screens, else GNOME's
    _shownOn(monitor) {
        const screens = this._ext.modules?.get('tiling')?.engine?.screens;
        return screens?.separate ? screens.shownOn(monitor) : null;
    }

    _goTo(workspace) {
        const time = global.get_current_time();
        const window = recentWindow(workspace);
        if (window)
            workspace.activate_with_focus(window, time);
        else
            workspace.activate(time);
    }

    // A notch of the wheel steps one workspace, up to the previous and down to the next, and stops at either end. Two
    // fingers on a touchpad do the same: GNOME turns their smooth scrolling into steps too, which count as notches.
    // The smooth events themselves and sideways scrolling are left to whatever else wants them.
    _scrolled(event) {
        let step;
        switch (event.get_scroll_direction()) {
        case Clutter.ScrollDirection.UP:
            step = -1;
            break;
        case Clutter.ScrollDirection.DOWN:
            step = 1;
            break;
        default:
            return Clutter.EVENT_PROPAGATE;
        }
        if (this._scrollPause)
            return Clutter.EVENT_STOP;

        // With a workspace per screen, the bar's screen goes through its own, as the page keys do: a workspace another
        // screen shows would take the pointer there, off the bar, and every later notch with it
        const screens = this._ext.modules?.get('tiling')?.engine?.screens;
        const monitor = Main.layoutManager.primaryIndex;
        const manager = global.workspace_manager;
        let target;
        if (screens?.separate) {
            const list = screens.workspacesFor(monitor, false);
            target = list[list.indexOf(screens.shownOn(monitor)) + step];
        } else {
            target = manager.get_workspace_by_index(manager.get_active_workspace_index() + step);
        }
        if (!target)
            return Clutter.EVENT_STOP;
        this._scrollPause = GLib.timeout_add(GLib.PRIORITY_DEFAULT, SCROLL_PAUSE_MS, () => {
            this._scrollPause = 0;
            return GLib.SOURCE_REMOVE;
        });
        if (screens?.separate)
            screens.bring(target, monitor);
        else
            this._goTo(target);
        return Clutter.EVENT_STOP;
    }

    // The keys move the workspace you are on by one place, as the row shows it: in a right-to-left language the row
    // runs from the right, so the left key moves it to a later place
    _shift(direction) {
        const step = this._row && isRtl(this._row) ? -direction : direction;
        const from = global.workspace_manager.get_active_workspace_index();
        this._move(from, from + step);
    }

    // A workspace moved to another place takes its windows and its name with it; the others close up behind it
    _move(from, to) {
        const manager = global.workspace_manager;
        if (from === to || to < 0 || to >= manager.n_workspaces)
            return;
        const names = this._namesFor(manager.n_workspaces);
        const [name] = names.splice(from, 1);
        names.splice(to, 0, name);
        this._moving = true;
        manager.reorder_workspace(manager.get_workspace_by_index(from), to);
        this._moving = false;
        this._writeNames(names);
    }

    // GNOME's list padded to one entry per workspace
    _namesFor(count) {
        const names = this._prefs.get_strv(NAMES);
        while (names.length < count)
            names.push('');
        return names;
    }

    _writeNames(names) {
        while (names.length > 0 && names.at(-1) === '')
            names.pop();
        this._prefs.set_strv(NAMES, names);
    }

    // The menu belongs to the row, beside the other menus of the top bar, but opens only from a label. The place-holder
    // GNOME gave the button leaves the top bar's menus, so that moving along the bar with a menu open finds this one.
    _buildMenu() {
        Main.panel.menuManager.removeMenu(this._row.menu);
        this._menu = new PopupMenu.PopupMenu(this._row, 0.5, St.Side.TOP);
        this._menu.actor.add_style_class_name('panel-menu');
        this._menu.actor.add_style_class_name('amethystora-workspaces-menu');
        Main.uiGroup.add_child(this._menu.actor);
        this._menu.actor.hide();
        Main.panel.menuManager.addMenu(this._menu);

        const heading = new PopupMenu.PopupBaseMenuItem({reactive: false, can_focus: false});
        heading.add_child(new St.Label({
            text: _('Name of this workspace'),
            style_class: 'amethystora-workspaces-heading',
        }));
        this._menu.addMenuItem(heading);

        const field = new PopupMenu.PopupBaseMenuItem({reactive: false, can_focus: false});
        this._entry = new St.Entry({style_class: 'amethystora-workspaces-entry', can_focus: true, x_expand: true});
        this._entry.clutter_text.connect('activate', () => this._menu.close());
        field.add_child(this._entry);
        this._menu.addMenuItem(field);

        // Back to the name of the workspace's main app, shown only while it has a name of its own
        this._unname = this._menu.addAction(_('Use the app’s name'), () => {
            this._entry.text = '';
        });

        // Connected after the top bar's own handler, which gives the menu the keyboard first
        this._menu.connect('open-state-changed', (menu, open) => {
            if (open) {
                this._row?.add_style_pseudo_class('active');
                // The label it was opened on, or the workspace you are on when opened from elsewhere
                const index = this._menuIndex ?? global.workspace_manager.get_active_workspace_index();
                this._menuIndex = null;
                const name = this._prefs.get_strv(NAMES)[index] ?? '';
                this._renaming = {index, name};
                this._entry.text = name;
                const workspace = global.workspace_manager.get_workspace_by_index(index);
                const app = this._settings?.get_boolean('auto-names') && workspace ? mainApp(workspace) : null;
                this._entry.hint_text = app?.get_name() ?? '';
                this._unname.visible = !!app && name !== '';
                this._checkAccent(index);
                this._setsMenu?.opened(workspace, this._byTouch);
                this._byTouch = false;
                global.stage.set_key_focus(this._entry.clutter_text);
                this._entry.clutter_text.set_selection(0, -1);
            } else {
                this._row?.remove_style_pseudo_class('active');
                this._rename();
            }
        });
    }

    // touch: a finger opened it
    openMenu(touch = false, index = null) {
        this._menuIndex = index;
        this._byTouch = touch;
        this._menu?.open(BoxPointer.PopupAnimation.FULL);
    }

    // ---- Saved sets (workspaces/sets.js, workspaces/setsmenu.js) ----

    _syncSets() {
        const on = this._settings?.get_boolean('sets') ?? false;
        if (on && !this.sets && this._menu) {
            this.sets = new Sets(this);
            try {
                this.sets.enable();
                this._setsMenu = new SetsMenu(this, this._menu);
            } catch (e) {
                console.error(`Amethystora UI: the saved sets did not start: ${e.message}`);
                this._stopSets();
            }
        } else if (!on) {
            this._stopSets();
        }
    }

    _stopSets() {
        this._setsMenu?.destroy();
        this._setsMenu = null;
        this.sets?.disable();
        this.sets = null;
    }

    // What the top bar calls a workspace: its own name, else its main app's while names come from apps, else ''
    titleOf(index, workspace) {
        const own = this._prefs?.get_strv(NAMES)[index] ?? '';
        if (own)
            return own;
        const app = this._settings?.get_boolean('auto-names') ? mainApp(workspace) : null;
        return app?.get_name() ?? '';
    }

    // A set opening on a workspace with no name of its own gives it the set's
    nameIfUnnamed(index, name) {
        if (!this._prefs || index < 0)
            return;
        const names = this._namesFor(index + 1);
        if (names[index])
            return;
        names[index] = name;
        this._writeNames(names);
    }

    // A name typed in the field counts at once when the workspace is saved, not only when the menu closes
    nameNow() {
        const renaming = this._renaming;
        if (!renaming || !this._entry || this._entry.text === renaming.name)
            return;
        this._rename();
        this._renaming = {index: renaming.index, name: this._entry.text};
    }

    openSet(name) {
        this._menu?.close(BoxPointer.PopupAnimation.FULL);
        this.sets?.open(name);
    }

    // For the tiling, places and the motion: a new window a set is opening
    claim(window) {
        return this.sets?.claim(window) ?? null;
    }

    claimed(window) {
        return this.sets?.claimed(window) ?? false;
    }

    arriving(window) {
        return this.sets?.arriving(window) ?? false;
    }

    // The Accent submenu: the desktop's own and the twelve, each with its colour, built only while each workspace has an
    // accent of its own
    _buildAccents() {
        this._accentMenu?.destroy();
        this._accentMenu = null;
        if (!this._menu || !this._themeSettings?.get_boolean('accent-per-workspace'))
            return;
        this._accentMenu = new PopupMenu.PopupSubMenuMenuItem(_('Accent'));
        this._accentItems = new Map();
        for (const [id, fill] of [['theme', null], ...ACCENTS]) {
            const item = new PopupMenu.PopupBaseMenuItem({style_class: 'amethystora-workspaces-accent'});
            const dot = new St.Widget({style_class: 'amethystora-workspaces-accent-dot', y_align: Clutter.ActorAlign.CENTER});
            // The twelve fills are the image's own, never text from elsewhere
            if (fill)
                dot.set_style(`background-color: ${fill};`);
            else
                dot.add_style_class_name('amethystora-workspaces-accent-theme');
            item.add_child(dot);
            const label = new St.Label({text: id === 'theme' ? _('Follow the theme') : accentName(id),
                y_align: Clutter.ActorAlign.CENTER});
            item.add_child(label);
            item.label_actor = label;
            item.connect('activate', () => this._chooseAccent(id));
            this._accentMenu.menu.addMenuItem(item);
            this._accentItems.set(id, item);
        }
        // Before the saved sets, which come last
        const before = this._setsMenu?.position ?? -1;
        this._menu.addMenuItem(this._accentMenu, before >= 0 ? before : undefined);
        this._accentMenu.connect('destroy', () => {
            this._accentMenu = null;
            this._accentItems = null;
        });
    }

    _checkAccent(index) {
        if (!this._accentItems)
            return;
        const current = workspaceAccent(this._themeSettings, index) ?? 'theme';
        for (const [id, item] of this._accentItems)
            item.setOrnament(id === current ? PopupMenu.Ornament.CHECK : PopupMenu.Ornament.NO_DOT);
    }

    // The workspace the menu was opened on takes the accent; the list grows with the desktop's own up to it
    _chooseAccent(id) {
        const index = this._renaming?.index ?? global.workspace_manager.get_active_workspace_index();
        const list = this._themeSettings.get_strv('workspace-accents');
        while (list.length <= index)
            list.push('theme');
        list[index] = id;
        this._themeSettings.set_strv('workspace-accents', list);
        this._checkAccent(index);
    }

    toggleMenu(index = null) {
        if (this._menu?.isOpen)
            this._menu.close(BoxPointer.PopupAnimation.FULL);
        else
            this.openMenu(false, index);
    }

    // However the menu closes, what the field then says is the name of the workspace it was opened on
    _rename() {
        const renaming = this._renaming;
        this._renaming = null;
        if (!renaming || !this._entry || this._entry.text === renaming.name)
            return;
        if (renaming.index >= global.workspace_manager.n_workspaces)
            return;
        const names = this._namesFor(renaming.index + 1);
        names[renaming.index] = this._entry.text;
        this._writeNames(names);
    }

    // ---- A window dragged onto a workspace ----

    // The glass under the note that says where a dragged window would go
    glassSurfaces() {
        return this._hint ? [{actor: this._hint, material: 'glass', radius: null}] : [];
    }

    _reduced() {
        return this._ext.modules?.get('comfort')?.reduceMotion ?? false;
    }

    // DND has no word for leaving a target: a monitor watches every move of the drag while it lasts
    _windowDragBegan() {
        if (this._monitor)
            return;
        this._monitor = {dragMotion: event => this._dragMotion(event)};
        DND.addDragMonitor(this._monitor);
    }

    _windowDragEnded() {
        if (this._monitor)
            DND.removeDragMonitor(this._monitor);
        this._monitor = null;
        this._setHit(null);
    }

    _dragMotion(event) {
        let label = null;
        for (let actor = event.targetActor; actor; actor = actor.get_parent()) {
            if (actor instanceof WorkspaceLabel) {
                label = this._labels?.includes(actor) ? actor : null;
                break;
            }
        }
        this._setHit(label);
        return DND.DragMotionResult.CONTINUE;
    }

    // The label a window is over lights up and grows a little, and the note under the bar names it
    _setHit(label) {
        if (label === this._hit)
            return;
        const previous = this._hit;
        this._hit = label;
        const grow = (actor, scale) => {
            actor.set_pivot_point(0.5, 0.5);
            if (this._reduced())
                actor.set_scale(1, 1);
            else
                actor.ease({scale_x: scale, scale_y: scale, duration: HIT_MS, mode: Clutter.AnimationMode.EASE});
        };
        if (previous && this._labels?.includes(previous)) {
            previous.remove_style_class_name('amethystora-workspaces-hit');
            grow(previous, 1);
        }
        if (!label) {
            if (this._hint) {
                this._hint.destroy();
                this._hint = null;
                this._ext.modules?.get('glass')?.surfacesChanged?.();
            }
            return;
        }
        label.add_style_class_name('amethystora-workspaces-hit');
        grow(label, HIT_SCALE);
        const made = !this._hint;
        if (made) {
            this._hint = new St.Label({style_class: 'amethystora-workspaces-hint'});
            this._hint.clutter_text.use_markup = false;
            Main.layoutManager.uiGroup.add_child(this._hint);
        }
        this._hint.text = _('Move to %s').format(label.labelText);
        this._placeHint(label);
        if (made)
            this._ext.modules?.get('glass')?.surfacesChanged?.();
    }

    // Centred under the label, a little below the top bar, and on the screen
    _placeHint(label) {
        const hint = this._hint;
        const [x] = label.get_transformed_position();
        const [width] = label.get_transformed_size();
        const [, natural] = hint.get_preferred_width(-1);
        const [panelX, panelY] = Main.panel.get_transformed_position();
        const [panelWidth, panelHeight] = Main.panel.get_transformed_size();
        const scale = St.ThemeContext.get_for_stage(global.stage).scale_factor;
        const left = Math.min(Math.max(x + width / 2 - natural / 2, panelX), panelX + panelWidth - natural);
        hint.set_position(Math.round(left), Math.round(panelY + panelHeight + HINT_GAP * scale));
        Main.layoutManager.uiGroup.set_child_above_sibling(hint, null);
    }

    // A window landed on a label: a count of the windows dropped there in the last moment shows on its end. The
    // workspace you are on stays on screen.
    landed(label) {
        const badge = label.badge;
        const count = (badge?.count ?? 0) + 1;
        if (badge) {
            if (badge.timer)
                GLib.source_remove(badge.timer);
            badge.actor.remove_all_transitions();
            badge.actor.opacity = 255;
        } else {
            const actor = new St.Label({style_class: 'amethystora-workspaces-badge', y_align: Clutter.ActorAlign.CENTER});
            label.add_child(actor);
            label.badge = {actor, count: 0, timer: 0};
        }
        label.badge.count = count;
        label.badge.actor.text = _('+%d').format(count);
        label.badge.timer = GLib.timeout_add(GLib.PRIORITY_DEFAULT,
            Math.max(1, Math.round(adjustAnimationTime(BADGE_MS, {animationRequired: true}))), () => {
                const held = label.badge;
                if (!held)
                    return GLib.SOURCE_REMOVE;
                held.timer = 0;
                held.actor.ease({
                    opacity: 0, duration: BADGE_FADE_MS, mode: Clutter.AnimationMode.EASE,
                    onStopped: () => this._dropBadge(label),
                });
                return GLib.SOURCE_REMOVE;
            });
    }

    _dropBadge(label) {
        const badge = label.badge;
        if (!badge)
            return;
        label.badge = null;
        if (badge.timer)
            GLib.source_remove(badge.timer);
        badge.actor.destroy();
    }

    // A label taken out of the row leaves a gap the size of itself where it was, so that nothing moves until the
    // drag goes somewhere else
    dragBegan(label) {
        const span = label.width + label.margin_left + label.margin_right;
        this._drag = {label, from: label.index, to: label.index, span, gap: null};
        label.add_style_class_name('amethystora-workspaces-dragged');
        this._drag.gap = this._openGap(label, span, false);
    }

    dragCancelled() {
        if (this._drag)
            this._closeGap(this._drag);
    }

    // The label is either gone, when it was dropped on the row and its workspace moved, or back at the end of the
    // row after its way home; either way it is made again in its place
    dragEnded(label) {
        if (!this._drag || this._drag.label !== label)
            return;
        const drag = this._drag;
        this._drag = null;
        this._closeGap(drag, false);
        if (this._labels.includes(label)) {
            label.remove_style_class_name('amethystora-workspaces-dragged');
            this._dropped = label;
        }
        this._queue();
    }

    _openGap(before, span, animate) {
        const gap = new St.Widget({style_class: 'amethystora-workspaces-gap', y_expand: true});
        if (before)
            this._row.box.insert_child_below(gap, before);
        else
            this._row.box.add_child(gap);
        if (animate) {
            gap.width = 0;
            gap.ease({width: span, duration: GAP_MS, mode: Clutter.AnimationMode.EASE_OUT_QUAD});
        } else {
            gap.width = span;
        }
        return gap;
    }

    _closeGap(drag, animate = true) {
        const gap = drag.gap;
        drag.gap = null;
        if (!gap)
            return;
        gap.remove_all_transitions();
        if (!animate) {
            gap.destroy();
            return;
        }
        gap.ease({
            width: 0,
            duration: GAP_MS,
            mode: Clutter.AnimationMode.EASE_OUT_QUAD,
            onStopped: () => gap.destroy(),
        });
    }

    // Where the dragged label would land: before the first other label whose middle its leading edge (the left one,
    // or the right one in a right-to-left language) has not passed yet, or after the last
    _landing(dragActor) {
        const rtl = isRtl(this._row);
        const [x] = dragActor.get_transformed_position();
        const [width] = dragActor.get_transformed_size();
        const lead = rtl ? x + width : x;
        const others = this._labels.filter(label => label !== this._drag.label);
        const at = others.findIndex(label => {
            const [labelX] = label.get_transformed_position();
            const [labelWidth] = label.get_transformed_size();
            const middle = labelX + labelWidth / 2;
            return rtl ? lead > middle : lead < middle;
        });
        return at < 0 ? others.length : at;
    }

    dragOver(source, dragActor) {
        if (!this._drag || source !== this._drag.label)
            return DND.DragMotionResult.NO_DROP;
        const to = this._landing(dragActor);
        if (to !== this._drag.to) {
            this._drag.to = to;
            this._closeGap(this._drag);
            const others = this._labels.filter(label => label !== this._drag.label);
            this._drag.gap = this._openGap(others[to] ?? null, this._drag.span, true);
        }
        return DND.DragMotionResult.MOVE_DROP;
    }

    drop(source, dragActor) {
        if (!this._drag || source !== this._drag.label)
            return false;
        const {from} = this._drag;
        const to = this._landing(dragActor);
        this._move(from, to);
        return true;
    }
}
