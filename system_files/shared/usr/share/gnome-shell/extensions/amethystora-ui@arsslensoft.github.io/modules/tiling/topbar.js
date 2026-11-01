// The tiling in the top bar: two small icons, one for how the workspace on the main screen scrolls to a window and
// one for where new windows open, each changed by a click and explained on hover; and the top bar hidden on the
// workspaces where the person hid it (Super+Ctrl+B), which then give its height to their windows. Off as the image
// ships, for whoever wants them: the workspace's name in the top bar (and each other screen's at its top left), and
// a line along the bottom of the top bar (at the top of each other screen) marking where the focused window is in
// its strip.

import Clutter from 'gi://Clutter';
import GObject from 'gi://GObject';
import Meta from 'gi://Meta';
import St from 'gi://St';

import {Extension, gettext as _} from 'resource:///org/gnome/shell/extensions/extension.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import * as PanelMenu from 'resource:///org/gnome/shell/ui/panelMenu.js';
import * as PopupMenu from 'resource:///org/gnome/shell/ui/popupMenu.js';

import {Mode} from './layout.js';
import {config} from './config.js';
import {Open} from './engine.js';
import {label as keyLabel} from './keys.js';
import {nameOf} from './backgrounds.js';
import {cardsShown} from './previews.js';

const MODE_ICONS = {
    [Mode.PLAIN]: 'sidebar-show-right-symbolic',
    [Mode.CENTRE]: 'preferences-desktop-multitasking-symbolic',
    [Mode.EDGE]: 'zoom-fit-best-symbolic',
};
// The way chosen has a colour in the tooltip, which the theme gives it (shell-tiling.css)
const MODE_TONES = {[Mode.PLAIN]: 'plain', [Mode.CENTRE]: 'centre', [Mode.EDGE]: 'edge'};
const OPEN_ICONS = {
    [Open.RIGHT]: 'go-next-symbolic',
    [Open.LEFT]: 'go-previous-symbolic',
    [Open.START]: 'go-first-symbolic',
    [Open.END]: 'go-last-symbolic',
    [Open.DOWN]: 'go-down-symbolic',
    [Open.UP]: 'go-up-symbolic',
};
// The tooltip, in pixels at the shell's scale: this far below the icon's top, so just under the bar, and its left
// edge at most this far left of the icon's
const TIP_BELOW = 34;
const TIP_LEFT = 62;
// The line that marks the focused window's place in its strip, and the least its mark is drawn
const POSITION_HEIGHT = 3;
const POSITION_MIN = 8;

const Icon = GObject.registerClass(
class Icon extends PanelMenu.Button {
    _init(name, styleClass, onClick, describe) {
        super._init(0.5, name, true);
        this.add_style_class_name(styleClass);
        this._icon = new St.Icon({style_class: 'system-status-icon'});
        this.add_child(this._icon);
        this._describe = describe;
        const click = new Clutter.ClickGesture({recognize_on_press: true});
        click.connect('recognize', () => {
            // Not over the overview or the workspace cards, which show the bar for what it is
            if (!Main.overview.visible && !cardsShown())
                onClick();
        });
        this.add_action(click);
        this.connect('notify::hover', () => this._tooltip());
        this.connect('destroy', () => this._tip?.destroy());
    }

    set iconName(name) {
        this._icon.icon_name = name;
        if (this._tip?.visible)
            this._fillTip();
    }

    _tooltip() {
        if (!this.hover) {
            this._tip?.hide();
            return;
        }
        if (!this._tip) {
            this._tip = new St.BoxLayout({
                orientation: Clutter.Orientation.VERTICAL, style_class: 'amethystora-tiling-tooltip',
            });
            this._lines = ['', 'amethystora-tiling-strong', 'amethystora-tiling-key', 'amethystora-tiling-note']
                .map(styleClass => {
                    const line = new St.Label({style_class: styleClass});
                    this._tip.add_child(line);
                    return line;
                });
            Main.uiGroup.add_child(this._tip);
        }
        this._fillTip();
        this._tip.show();
        const [x, y] = this.get_transformed_position();
        const [, height] = this.get_transformed_size();
        const [, tipWidth] = this._tip.get_preferred_width(-1);
        const monitor = Main.layoutManager.findMonitorForActor(this);
        const scale = St.ThemeContext.get_for_stage(global.stage).scale_factor;
        const left = Math.max(monitor.x, Math.round(x - TIP_LEFT * scale));
        this._tip.set_position(Math.min(left, monitor.x + monitor.width - tipWidth),
            Math.round(y + Math.max(height, TIP_BELOW * scale)));
    }

    // The tooltip says what the icon is about, the way now chosen (in the colour its tone has, when it has one), the
    // key that changes it, and a note when there is one, a line each
    _fillTip() {
        const {heading, current, tone, key, note} = this._describe();
        const [first, second, third, fourth] = this._lines;
        first.text = heading;
        second.text = current;
        second.style_class = tone ? `amethystora-tiling-tone-${tone}` : 'amethystora-tiling-strong';
        third.text = key ?? '';
        third.visible = !!key;
        fourth.text = note ?? '';
        fourth.visible = !!note;
    }
});

// The name of the workspace on the main screen: a click shows the overview, the other button opens a menu to rename
// it, and the wheel goes to the workspace before or after it
const Indicator = GObject.registerClass(
class Indicator extends PanelMenu.Button {
    _init(topBar) {
        super._init(0.0, _('Workspace name'));
        this._topBar = topBar;
        this.add_style_class_name('amethystora-tiling-indicator');
        this._label = new St.Label({y_align: Clutter.ActorAlign.CENTER});
        this.add_child(this._label);
        // Taken before the button's own click, which opens the menu with any other button
        const overview = new Clutter.ClickGesture({required_button: Clutter.BUTTON_PRIMARY, recognize_on_press: true});
        overview.connect('recognize', () => {
            this.menu.close();
            if (Main.overview.shouldToggleByCornerOrButton())
                Main.overview.toggle();
        });
        this.add_action_full('amethystora-tiling-indicator-overview', Clutter.EventPhase.CAPTURE, overview);

        this._entry = new St.Entry({
            style_class: 'amethystora-tiling-rename', hint_text: _('Name of this workspace'), can_focus: true,
            x_expand: true,
        });
        const item = new PopupMenu.PopupBaseMenuItem({reactive: false, can_focus: false});
        item.add_child(this._entry);
        this.menu.addMenuItem(item);
        this.menu.addMenuItem(new PopupMenu.PopupSeparatorMenuItem());
        this.menu.addAction(_('Tiling settings'), () => Extension.lookupByURL(import.meta.url)?.openPreferences());
        this._entry.clutter_text.connect('activate', () => this.menu.close());
        // The name is kept when the menu closes, however it closes
        this.menu.connect('open-state-changed', (menu, open) => {
            if (open) {
                this._renaming = topBar.primaryWorkspace();
                this._entry.text = config.workspaceNames[this._renaming.index()] ?? '';
                global.compositor.get_laters().add(Meta.LaterType.BEFORE_REDRAW, () => {
                    if (this._entry && this.menu.isOpen)
                        global.stage.set_key_focus(this._entry);
                    return false;
                });
            } else if (this._renaming && this._entry) {
                topBar.rename(this._renaming, this._entry.text);
                this._renaming = null;
            }
        });
        this.connect('scroll-event', (actor, event) => this._scroll(event));
        this.connect('destroy', () => {
            this._entry = null;
        });
    }

    set text(text) {
        this._label.text = text;
    }

    _scroll(event) {
        const step = {
            [Clutter.ScrollDirection.UP]: -1, [Clutter.ScrollDirection.LEFT]: -1,
            [Clutter.ScrollDirection.DOWN]: 1, [Clutter.ScrollDirection.RIGHT]: 1,
        }[event.get_scroll_direction()];
        const manager = global.workspace_manager;
        const target = step ? manager.get_workspace_by_index(this._topBar.primaryWorkspace().index() + step) : null;
        if (target && !Main.overview.visible)
            target.activate(global.get_current_time());
        return Clutter.EVENT_STOP;
    }
});

export class TopBar {
    constructor(tiling) {
        this._tiling = tiling;
    }

    enable() {
        this._mode = new Icon(_('Focus mode'), 'amethystora-tiling-mode',
            () => this._tiling.actions.cycleFocusMode(), () => this._describeMode());
        this._open = new Icon(_('Where new windows open'), 'amethystora-tiling-open',
            () => this._tiling.actions.cycleOpenPosition(), () => this._describeOpen());
        // Right after the workspaces' numbers, where they are: the tiling turned on again after the classic layout comes
        // after more of the bar than it does at a login
        const row = Main.panel.statusArea['amethystora-workspaces']?.container;
        const after = row ? Main.panel._leftBox.get_children().indexOf(row) + 1 : 0;
        const at = after > 0 ? after : 2;
        Main.panel.addToStatusArea('amethystora-tiling-mode', this._mode, at, 'left');
        Main.panel.addToStatusArea('amethystora-tiling-open', this._open, at + 1, 'left');
        // The workspaces the person turned the other way, kept by the workspace itself and written down by its number
        this._turned = {top: this._load('hiddenTopBar'), position: this._load('hiddenPositionBar')};
        this._screens = new Map();
        global.workspace_manager.connectObject(
            'active-workspace-changed', () => {
                this.sync();
                this.applyTopBar();
            },
            'workspaces-reordered', () => this._keep(),
            'workspace-removed', () => this._keep(),
            this);

        // GNOME hides the top bar (and whatever else gives way to fullscreen windows) while a fullscreen window is
        // on the screen; a fullscreen window the strip has scrolled away is not, and the bar comes back. The top
        // bar also stays hidden on the workspaces where the person hid it, but for the workspace cards.
        const layout = Main.layoutManager;
        const original = layout._updateActorVisibility;
        if (typeof original === 'function') {
            this._ownVisibility = Object.prototype.hasOwnProperty.call(layout, '_updateActorVisibility')
                ? original : null;
            const self = this;
            layout._updateActorVisibility = function (actorData) {
                original.call(this, actorData);
                if (!global.window_group.visible)
                    return;
                const monitor = actorData.trackFullscreen ? this.findMonitorForActor(actorData.actor) : null;
                if (monitor?.inFullscreen && !self._tiling.engine?.coversMonitor(monitor.index))
                    actorData.actor.visible = true;
                if (actorData.actor === this.panelBox && cardsShown())
                    actorData.actor.visible = true;
                else if (actorData.actor === this.panelBox && self.hidden())
                    actorData.actor.visible = false;
            };
            this._patched = true;
        }
        this.sync();
        this.applyTopBar();
    }

    disable() {
        global.workspace_manager.disconnectObject(this);
        this._watch(false);
        if (this._patched) {
            if (this._ownVisibility)
                Main.layoutManager._updateActorVisibility = this._ownVisibility;
            else
                delete Main.layoutManager._updateActorVisibility;
            this._patched = false;
        }
        this.applyTopBar(false);
        if (this._later)
            global.compositor.get_laters().remove(this._later);
        this._later = 0;
        for (const shown of this._screens?.values() ?? [])
            Object.values(shown).forEach(actor => actor.destroy());
        this._screens = null;
        for (const part of ['_mode', '_open', '_indicator']) {
            this[part]?.destroy();
            this[part] = null;
        }
    }

    _primaryStrip() {
        return this._tiling.engine.activeStrip(Main.layoutManager.primaryIndex);
    }

    // The workspace the main screen shows, which the top bar is about
    primaryWorkspace() {
        return this._tiling.engine?.screens?.shownOn(Main.layoutManager.primaryIndex) ??
            global.workspace_manager.get_active_workspace();
    }

    sync() {
        if (!this._mode)
            return;
        this._mode.iconName = MODE_ICONS[this._primaryStrip().mode];
        this._open.iconName = OPEN_ICONS[this._tiling.openPosition] ?? OPEN_ICONS[Open.RIGHT];
        this._mode.visible = config.modeIcon;
        this._open.visible = config.openIcon;
        if (config.indicator && !this._indicator) {
            this._indicator = new Indicator(this);
            Main.panel.addToStatusArea('amethystora-tiling-workspace', this._indicator, 1, 'left');
        } else if (!config.indicator && this._indicator) {
            this._indicator.destroy();
            this._indicator = null;
        }
        if (this._indicator)
            this._indicator.text = nameOf(this.primaryWorkspace());
        this._watch(config.indicator || config.positionBar);
        this.positionsChanged();
    }

    _key(name) {
        return keyLabel(this._tiling.keys.first(name));
    }

    _describeMode() {
        const mode = this._primaryStrip().mode;
        const name = {
            [Mode.PLAIN]: _('Keep in view'),
            [Mode.CENTRE]: _('Centre the focused window'),
            [Mode.EDGE]: _('Push to the nearest edge'),
        }[mode];
        return {
            heading: _('How this workspace scrolls to a window'), current: name, tone: MODE_TONES[mode],
            key: this._key('switch-focus-mode'),
        };
    }

    _describeOpen() {
        const name = {
            [Open.RIGHT]: _('To the right'),
            [Open.LEFT]: _('To the left'),
            [Open.START]: _('At the start of the strip'),
            [Open.END]: _('At the end of the strip'),
            [Open.DOWN]: _('Below'),
            [Open.UP]: _('Above'),
        }[this._tiling.openPosition] ?? _('To the right');
        return {
            heading: _('Where new windows open'), current: name, key: this._key('switch-open-window-position'),
            note: _('The places a click goes through are chosen in the tiling settings'),
        };
    }

    // ---- The top bar and the position line per workspace ----

    _load(key) {
        const manager = global.workspace_manager;
        const indices = this._tiling.state?.get(key);
        return new Set((Array.isArray(indices) ? indices : []).filter(Number.isInteger)
            .map(index => manager.get_workspace_by_index(index)).filter(Boolean));
    }

    // The workspaces were reordered, or one went: the lists follow the workspaces themselves
    _keep() {
        const manager = global.workspace_manager;
        const all = new Set(Array.from({length: manager.n_workspaces}, (u, i) => manager.get_workspace_by_index(i)));
        for (const set of Object.values(this._turned)) {
            for (const workspace of set) {
                if (!all.has(workspace))
                    set.delete(workspace);
            }
        }
        this._save();
        this.sync();
        this.applyTopBar();
    }

    _save() {
        const indices = set => [...set].map(w => w.index()).filter(i => i >= 0).sort((a, b) => a - b);
        this._tiling.state?.set('hiddenTopBar', indices(this._turned.top));
        this._tiling.state?.set('hiddenPositionBar', indices(this._turned.position));
    }

    // The top bar is on the primary screen, so the workspace that screen shows decides
    hidden() {
        return this.hiddenOn(this.primaryWorkspace());
    }

    // The workspaces in the list are the ones the person turned the other way from the setting's default
    hiddenOn(workspace) {
        const turned = this._turned?.top.has(workspace) ?? false;
        return config.topBarDefault ? turned : !turned;
    }

    // `which`: 'top', 'position', or 'both', the position line then following the top bar. On the workspace a
    // navigation has chosen, else on GNOME's.
    toggle(which = 'both') {
        const workspace = this._tiling.engine?.session?.strip?.workspace ??
            global.workspace_manager.get_active_workspace();
        const flip = set => set.has(workspace) ? set.delete(workspace) : set.add(workspace);
        const {top, position} = this._turned;
        if (which !== 'position')
            flip(top);
        if (which === 'position')
            flip(position);
        else if (which === 'both' && this.hiddenOn(workspace))
            position.add(workspace);
        else if (which === 'both')
            position.delete(workspace);
        this._save();
        this.applyTopBar();
    }

    // The bar hidden takes its space on the screen with it, so that the windows can have it
    applyTopBar(hidden = this.hidden()) {
        const layout = Main.layoutManager;
        const data = layout._trackedActors?.find(a => a.actor === layout.panelBox);
        if (data && data.affectsStruts === hidden) {
            data.affectsStruts = !hidden;
            layout._queueUpdateRegions();
        }
        layout._updateVisibility();
        this.positionsChanged();
    }

    // ---- The other screens' names, and the position line ----

    // Watched only while one of them is on
    _watch(on) {
        if (on === !!this._watching)
            return;
        this._watching = on;
        const signals = [
            [global.display, 'notify::focus-window'], [global.display, 'restacked'],
            [global.window_manager, 'size-changed'], [Main.layoutManager, 'monitors-changed'],
            [Main.overview, 'showing'], [Main.overview, 'hidden'], [Main.layoutManager.panelBox, 'notify::visible'],
            [this._tiling.settings, 'changed::workspace-names'],
        ];
        for (const [object, signal] of signals) {
            if (on)
                object?.connectObject(signal, () => this.positionsChanged(), this);
            else
                object?.disconnectObject(this);
        }
        if (!on && this._later) {
            global.compositor.get_laters().remove(this._later);
            this._later = 0;
        }
    }

    // Drawn again before the next frame, once however many things changed
    positionsChanged() {
        if (!this._screens || this._later)
            return;
        if (!this._watching && !this._screens.size)
            return;
        this._later = global.compositor.get_laters().add(Meta.LaterType.BEFORE_REDRAW, () => {
            this._later = 0;
            this._drawScreens();
            return false;
        });
    }

    _drawScreens() {
        if (!this._screens || !this._tiling.engine)
            return;
        const layout = Main.layoutManager;
        if (this._indicator)
            this._indicator.text = nameOf(this.primaryWorkspace());
        const scale = St.ThemeContext.get_for_stage(global.stage).scale_factor;
        layout.monitors.forEach((monitor, index) => {
            const shown = this._screens.get(index) ?? {};
            this._screens.set(index, shown);
            const workspace = this._tiling.engine.screens?.shownOn(index) ?? global.workspace_manager.get_active_workspace();
            const visible = !Main.overview.visible;
            // A screen with a top bar of its own (modules/bars.js) shows its workspaces there rather than the name
            const bar = this._tiling._ext.modules.get('bars')?.barOn?.(index) ?? null;
            // The name, on the screens the top bar is not on
            const named = config.indicator && index !== layout.primaryIndex && visible && !bar;
            this._chrome(shown, 'name', named, () => new St.Label({style_class: 'amethystora-tiling-screen-name'}));
            if (shown.name) {
                shown.name.text = nameOf(workspace);
                shown.name.set_position(monitor.x + 12 * scale, monitor.y + 6 * scale);
            }
            this._drawPosition(shown, index, monitor, workspace, visible, scale, bar);
        });
        for (const [index, shown] of this._screens) {
            if (index >= layout.monitors.length || !Object.keys(shown).length) {
                Object.values(shown).forEach(actor => actor.destroy());
                this._screens.delete(index);
            }
        }
    }

    // The line across the screen, and on it the mark of the focused window's column, as wide as the column is of the
    // whole strip
    _drawPosition(shown, index, monitor, workspace, visible, scale, bar = null) {
        const strip = this._tiling.engine.activeStrip(index);
        const at = strip.selected ? strip.find(strip.selected) : null;
        const wanted = config.positionBar && visible && !!at && !this._turned.position.has(workspace);
        this._chrome(shown, 'line', wanted, () => {
            const line = new St.Widget({style_class: 'amethystora-tiling-position'});
            line.add_child(new St.Widget({style_class: 'amethystora-tiling-position-mark'}));
            return line;
        });
        if (!shown.line)
            return;
        const panel = Main.layoutManager.panelBox;
        const height = POSITION_HEIGHT * scale;
        const onBar = index === Main.layoutManager.primaryIndex && panel.visible;
        // Along the bottom of the main bar, or of the screen's own bar, else at the top of the screen
        let y = monitor.y;
        if (onBar)
            y = panel.y + panel.height - height;
        else if (bar?.visible)
            y = bar.y + bar.height - height;
        shown.line.set_position(monitor.x, y);
        shown.line.set_size(monitor.width, height);
        const geo = this._tiling.engine.geometry(strip);
        const column = geo.columns[at[0]];
        const share = monitor.width / Math.max(1, geo.width);
        const width = Math.max(POSITION_MIN * scale, Math.round(column.width * share));
        const mark = shown.line.get_first_child();
        mark.set_position(Math.min(monitor.width - width, Math.round(column.x * share)), 0);
        mark.set_size(width, height);
    }

    // An actor of a screen's, made when wanted and taken away when not; it gives way to fullscreen windows
    _chrome(shown, part, wanted, make) {
        if (wanted && !shown[part]) {
            shown[part] = make();
            Main.layoutManager.addChrome(shown[part], {trackFullscreen: true});
        } else if (!wanted && shown[part]) {
            shown[part].destroy();
            delete shown[part];
        }
    }

    // A workspace's name, as the person typed it; an empty one is "Workspace N" again
    rename(workspace, text) {
        const index = workspace.index();
        const names = this._tiling.settings.get_strv('workspace-names');
        if (index < 0 || (names[index] ?? '') === text.trim())
            return;
        while (names.length <= index)
            names.push('');
        names[index] = text.trim();
        while (names.length && !names.at(-1))
            names.pop();
        this._tiling.settings.set_strv('workspace-names', names);
    }
}
