// Quick settings as live tiles. The menu at the right end of the top bar keeps GNOME's own tiles, sliders and buttons,
// laid out again: the tiles first, two to a row, each saying what it knows (the Wi-Fi network and its strength, the
// earbuds and their battery, when the battery runs out, the phone and its battery); then GNOME's sliders, each app's own
// volume while something plays, a swatch for every theme, and Screenshot, Lock, Log out, Settings and Power in a row at
// the bottom. Holding a tile arranges them: tiles can be moved and put away (modules/quick/arrange.js), which the keys
// tile-order and tile-hidden of org.gnome.shell.extensions.amethystora-ui.quick keep (`ame desktop quick-settings`).
//
// GNOME's items stay GNOME's: they are put in another order, given other words beside GNOME's and styled, never made
// again, and everything goes back as it was when the module stops. GNOME's battery button, which the battery's tile
// replaces, is only taken out of the row and kept alive, since it still feeds the top bar's percentage. A tile put away
// is taken out of the grid (GNOME's code still updates it) rather than hidden, since GNOME sets each tile's visibility
// itself. Nothing here runs while quick settings is closed but what GNOME runs anyway: the words, the battery's
// changes, the themes and the apps' streams are followed only while it is open.
//
// For the VM suite only, the module answers what a person would see and does what a person would do, through the same
// code: tileIds(), hiddenIds(), arrange(on), hideForTest(id), moveForTest(id, beforeId), themeSwatches(), pickTheme(slug),
// appRows(), setAppRowVolume(name, fraction), muteForTest(name), outputs(), pickOutput(description), batteryRow(),
// batterySubtitle() and toggleBatteryLimit().

import Clutter from 'gi://Clutter';
import GObject from 'gi://GObject';
import Meta from 'gi://Meta';
import St from 'gi://St';

import {gettext as _} from 'resource:///org/gnome/shell/extensions/extension.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import {QuickSettingsItem, QuickToggle} from 'resource:///org/gnome/shell/ui/quickSettings.js';
import * as SystemActions from 'resource:///org/gnome/shell/misc/systemActions.js';

import {Arrange} from './quick/arrange.js';
import {BatteryTile} from './quick/battery.js';
import {ThemeBlock} from './quick/theme.js';
import {ID, PhoneTile, Subtitles, isTile, tileId} from './quick/tiles.js';
import {VolumeBlock} from './quick/volume.js';

const SCHEMA = 'org.gnome.shell.extensions.amethystora-ui.quick';
// GNOME's items that are no tiles, by their class: the sliders, the row of buttons and the background apps
const BRIGHTNESS = 'BrightnessItem';
const OUTPUT = 'OutputStreamSlider';
const INPUT = 'InputStreamSlider';
const SYSTEM = 'SystemItem';
const BACKGROUND = 'BackgroundAppsToggle';
// GNOME's buttons in the row, in the board's order, each with its word
const ACTIONS = ['ScreenshotItem', 'LockItem', 'SettingsItem', 'ShutdownItem'];
// How long a tile is held before arrange mode starts
const HOLD_MS = 500;
// A tile put away, as the Hidden row shows it while arranging
const HIDDEN_OPACITY = 128;
// What a block of two columns asks for: GNOME's grid halves a block's width to find a column's, leaving out the gap
// between the columns, so a block asking for the whole row (184 + 10 + 184) would widen every tile by 5
const BLOCK_WIDTH = 2 * 184;

export class Quick {
    constructor(ext) {
        this._ext = ext;
    }

    enable() {
        const quickSettings = Main.panel.statusArea.quickSettings;
        this.menu = quickSettings.menu;
        this.grid = this.menu._grid;
        this._placeholder = this.grid.layout_manager._overlay;
        this._settings = this._ext.getSettings(SCHEMA);

        // The default order: GNOME's as it is now; tiles that come later after them, as they come
        this._rank = new Map();
        this._gnome = new Set();
        this._original = this.grid.get_children();
        const beforePlaceholder = this._original.indexOf(this._placeholder);
        this._original.forEach((actor, i) => {
            this._rank.set(actor, i);
            if (beforePlaceholder < 0 || i < beforePlaceholder)
                this._gnome.add(actor);
        });
        this._nextRank = 1000;
        this._late = [];
        this._known = new Set();
        this._held = new Set();

        this.menu.box.add_style_class_name('amethystora-quick');
        this._subtitles = new Subtitles();
        this._arrange = new Arrange(this);

        // Amethystora's own items
        this._battery = new BatteryTile();
        this._insert(this._battery.tile, 1, true);
        this._rank.set(this._battery.tile, -1);
        this._phone = new PhoneTile(this._ext.modules?.get('connect') ?? null);
        this._insert(this._phone.tile, 1, true);
        this._theme = new ThemeBlock();
        this._insert(this._theme.block, 2);
        this._volume = new VolumeBlock();
        this._insert(this._volume.block, 2);
        this._buildHint();
        this._buildHiddenRow();

        this._settings.connectObject(
            'changed::tile-order', () => this._layout(),
            'changed::tile-hidden', () => this._layout(),
            this);
        this.grid.connectObject(
            'child-added', (grid, child) => this._childAdded(child),
            this);
        this.menu.connectObject('open-state-changed', (menu, open) => this._openChanged(open), this);

        this._layout();
        if (this.menu.isOpen)
            this._openChanged(true);
    }

    disable() {
        this._arrange?.leave();
        this._arrange = null;
        if (this._later) {
            global.compositor.get_laters().remove(this._later);
            this._later = 0;
        }
        this._settings?.disconnectObject(this);
        this.grid?.disconnectObject(this);
        this.menu?.disconnectObject(this);
        this._subtitles?.destroy();
        this._subtitles = null;

        for (const actor of this._held ?? []) {
            actor.disconnectObject(this);
            const gesture = actor._amethystoraQuickHold;
            if (gesture) {
                actor.remove_action(gesture);
                delete actor._amethystoraQuickHold;
            }
        }
        this._held?.clear();

        // The tiles put away go back into the grid (their place is set below)
        for (const actor of this._known ?? []) {
            const ours = actor === this._battery?.tile || actor === this._phone?.tile;
            if (actor.get_parent() !== this.grid && !ours) {
                actor.get_parent()?.remove_child(actor);
                this._insert(actor, 1);
            }
            actor.set_scale?.(1, 1);
            actor.set_translation?.(0, 0, 0);
            if (actor.opacity === 0)
                actor.opacity = 255;
        }
        this._known?.clear();

        this._battery?.destroy();
        this._battery = null;
        this._phone?.destroy();
        this._phone = null;
        this._theme?.destroy();
        this._theme = null;
        this._volume?.destroy();
        this._volume = null;
        this._hint?.destroy();
        this._hint = null;
        this._hiddenRow?.destroy();
        this._hiddenRow = null;
        this._themeStandIn?.destroy();
        this._themeStandIn = null;
        if (this._focusBlock?.get_parent() === this.grid)
            this.grid.remove_child(this._focusBlock);
        this._focusBlock = null;
        if (this._screensBlock?.get_parent() === this.grid)
            this.grid.remove_child(this._screensBlock);
        this._screensBlock = null;
        this._releaseSystem();
        this._restoreOrder();
        this.menu?.box.remove_style_class_name('amethystora-quick');
        this.menu = null;
        this.grid = null;
        // Disposed now rather than by the collector (dock.js says why)
        this._settings?.run_dispose();
        this._settings = null;
        this._rank = null;
        this._gnome = null;
        this._original = null;
        for (const actor of this._late ?? [])
            actor.disconnectObject(this);
        this._late = null;
    }

    // GNOME's order back: its own items in the order they had, the ones that came later where GNOME puts such items
    _restoreOrder() {
        const grid = this.grid;
        // Nothing was moved when enable stopped before it knew GNOME's order
        if (!grid || !this._original)
            return;
        const order = this._original.filter(actor => actor.get_parent() === grid);
        // In the order they came: GNOME's own before its placeholder, other extensions' before the background apps
        for (const actor of this._late ?? []) {
            if (actor.get_parent() !== grid || order.includes(actor))
                continue;
            const at = this._gnome.has(actor) ? order.indexOf(this._placeholder)
                : order.findIndex(each => each.constructor?.name === BACKGROUND);
            order.splice(at < 0 ? order.length : at, 0, actor);
        }
        order.forEach((actor, i) => grid.set_child_at_index(actor, i));
    }

    _insert(actor, span, ours = false) {
        this._inserting = true;
        try {
            this.grid.add_child(actor);
            this.grid.layout_manager.child_set_property(this.grid, actor, 'column-span', span);
            if (span === 2 && !this._known.has(actor))
                actor.set({min_width: 0, natural_width: BLOCK_WIDTH});
        } finally {
            this._inserting = false;
        }
        if (ours && actor.menu)
            this._connectToggleMenu(actor);
    }

    // A tile's own menu dims quick settings while it is open, as GNOME's do
    _connectToggleMenu(actor) {
        this.menu._overlay.add_child(actor.menu.actor);
        actor.menu.connect('open-state-changed', (m, open) => {
            this.menu._setDimmed?.(open);
            this.menu._activeMenu = open ? actor.menu : null;
        });
    }

    _childAdded(child) {
        if (this._inserting || this._laying)
            return;
        if (!this._rank.has(child)) {
            const children = this.grid.get_children();
            this._rank.set(child, this._nextRank++);
            // GNOME puts its own items before the placeholder of its menus, other extensions' after it
            if (children.indexOf(child) < children.indexOf(this._placeholder))
                this._gnome.add(child);
            this._late.push(child);
            // A tile that goes while this runs (a module's own, turned off) is forgotten, so that nothing touches it again
            child.connectObject('destroy', () => this._forget(child), this);
        }
        // A tile that came later is put in its place once, before the next frame
        if (!this._later) {
            this._later = global.compositor.get_laters().add(Meta.LaterType.BEFORE_REDRAW, () => {
                this._later = 0;
                this._layout();
                return false;
            });
        }
    }

    _forget(actor) {
        this._late = this._late?.filter(each => each !== actor) ?? null;
        this._known?.delete(actor);
        this._held?.delete(actor);
        this._rank?.delete(actor);
        this._gnome?.delete(actor);
    }

    _isGnome(actor) {
        return this._gnome?.has(actor) ?? false;
    }

    _ids(key) {
        return this._settings.get_strv(key).filter(id => ID.test(id));
    }

    idOf(actor) {
        if (actor === this._theme?.block || actor === this._themeStandIn)
            return 'theme';
        return tileId(actor, this._isGnome(actor));
    }

    titleOf(actor) {
        if (actor === this._theme?.block || actor === this._themeStandIn)
            return _('Theme');
        return actor.title || this.idOf(actor);
    }

    // --- The layout -------------------------------------------------------------------------------------------------

    _byClass(name) {
        return this.grid.get_children().find(actor => actor.constructor?.name === name) ?? null;
    }

    // Every tile there is: GNOME's, ours and other extensions', in the grid or put away
    _collect() {
        for (const child of this.grid.get_children()) {
            if (child === this._placeholder || this._known.has(child) || !isTile(this.grid, child))
                continue;
            this._known.add(child);
            // A tile another module takes away while this one runs (the live captions' toggle, the privacy
            // module's shield) is forgotten whole, so that putting GNOME's order back does not touch it
            child.connectObject('destroy', () => this._forget(child), this);
            this._hold(child);
        }
        this._subtitles.adopt(this._known);
    }

    // Holding a tile (or Menu on it) starts arrange mode; the press that starts it toggles nothing
    _hold(actor) {
        if (this._held.has(actor))
            return;
        const gesture = new Clutter.LongPressGesture({long_press_duration_ms: HOLD_MS});
        gesture.connect('recognize', () => this.arrange(true, actor));
        actor.add_action(gesture);
        actor._amethystoraQuickHold = gesture;
        actor.connectObject('popup-menu', () => {
            actor.menu?.close();
            if (this.menu.isOpen)
                this._arrange.enter(actor);
        }, this);
        this._held.add(actor);
    }

    _layout() {
        // Nothing once the module stopped, or once GNOME's grid is gone (a destroyed actor has no layout manager left):
        // a later queued before either still comes
        if (!this.grid?.layout_manager || this._laying)
            return;
        this._laying = true;
        try {
            this._collect();
            this._adoptSystem();
            const grid = this.grid;
            const hidden = new Set(this._ids('tile-hidden'));
            const arranging = this._arrange?.active ?? false;

            // Put away, and back
            for (const actor of this._known) {
                const away = hidden.has(this.idOf(actor));
                if (away && actor.get_parent() === grid) {
                    grid.remove_child(actor);
                } else if (!away && actor.get_parent() !== grid) {
                    actor.get_parent()?.remove_child(actor);
                    this._insert(actor, 1);
                    actor.opacity = 255;
                }
            }
            const themeAway = hidden.has('theme');
            if (themeAway && this._theme.block.get_parent() === grid)
                grid.remove_child(this._theme.block);
            else if (!themeAway && !this._theme.block.get_parent())
                this._insert(this._theme.block, 2);

            // The Hidden row, while arranging and something is put away
            const away = [...this._known].filter(actor => actor.get_parent() !== grid && actor.visible);
            for (const actor of away) {
                if (arranging && actor.get_parent() !== this._hiddenBox) {
                    actor.get_parent()?.remove_child(actor);
                    this._hiddenBox.add_child(actor);
                    actor.opacity = HIDDEN_OPACITY;
                } else if (!arranging && actor.get_parent() === this._hiddenBox) {
                    this._hiddenBox.remove_child(actor);
                    actor.opacity = 255;
                }
            }
            if (themeAway && arranging) {
                if (this._themeStandIn.get_parent() !== this._hiddenBox)
                    this._hiddenBox.add_child(this._themeStandIn);
            } else if (this._themeStandIn.get_parent()) {
                this._hiddenBox.remove_child(this._themeStandIn);
            }
            this._placeHidden();
            const showRow = arranging && this._hiddenBox.get_n_children() > 0;
            if (showRow && this._hiddenRow.get_parent() !== grid)
                this._insert(this._hiddenRow, 2);
            else if (!showRow && this._hiddenRow.get_parent() === grid)
                grid.remove_child(this._hiddenRow);

            // The tiles first: those the key names in its order, then the rest as GNOME has them
            const order = this._ids('tile-order');
            const position = actor => {
                const i = order.indexOf(this.idOf(actor));
                return i < 0 ? Infinity : i;
            };
            const tiles = [...this._known].filter(actor => actor.get_parent() === grid)
                .sort((a, b) => position(a) - position(b) || this._defaultRank(a) - this._defaultRank(b));

            const fixed = [BRIGHTNESS, OUTPUT, INPUT, SYSTEM, BACKGROUND].map(name => this._byClass(name));
            const [brightness, output, input, system, background] = fixed;
            const placed = new Set([...tiles, ...fixed, this._placeholder, this._hiddenRow, this._theme.block,
                this._volume.block, this._hint, this._focusBlock, this._screensBlock]);
            const others = grid.get_children().filter(actor => !placed.has(actor));
            const sequence = [
                ...tiles,
                this._hiddenRow,
                this._focusBlock,
                ...others,
                brightness,
                this._screensBlock,
                output,
                this._volume.block,
                input,
                this._theme.block,
                system,
                this._hint,
                this._placeholder,
                background,
            ].filter(actor => actor && actor.get_parent() === grid);
            sequence.forEach((actor, i) => {
                if (grid.get_child_at_index(i) !== actor)
                    grid.set_child_at_index(actor, i);
            });
        } finally {
            this._laying = false;
        }
        this._arrange?.refresh();
    }

    // The Focus tile (modules/focus/tile.js), or null: a block across both columns after the tiles, while the focus
    // module runs. It is not one of the tiles arrange mode moves or puts away.
    setFocusBlock(actor) {
        if (this._focusBlock === actor)
            return;
        if (this.grid && this._focusBlock?.get_parent() === this.grid)
            this.grid.remove_child(this._focusBlock);
        this._focusBlock = actor;
        if (actor && this.grid) {
            this._insert(actor, 2);
            this._layout();
        }
    }

    // The external screens' brightness (modules/brightness.js), or null: a block across both columns after GNOME's
    // brightness slider, while the brightness module's screens are on
    setScreensBlock(actor) {
        if (this._screensBlock === actor)
            return;
        if (this.grid && this._screensBlock?.get_parent() === this.grid)
            this.grid.remove_child(this._screensBlock);
        this._screensBlock = actor;
        if (actor && this.grid) {
            this._insert(actor, 2);
            this._layout();
        }
    }

    // Where a tile goes by default: the battery first, the phone after Bluetooth, the rest in GNOME's order
    _defaultRank(actor) {
        if (actor === this._phone?.tile) {
            const bluetooth = [...this._known].find(a => this.idOf(a) === 'bluetooth');
            return bluetooth ? this._defaultRank(bluetooth) + 0.5 : this._nextRank;
        }
        return this._rank.get(actor) ?? this._nextRank;
    }

    // --- Screenshot, Lock, Settings and Power ----------------------------------------------------------------------

    _adoptSystem() {
        if (this._system)
            return;
        const item = this._byClass(SYSTEM);
        const box = item?.child;
        if (!box)
            return;
        const children = box.get_children();
        const buttons = ACTIONS.map(name => children.find(child => child.constructor?.name === name));
        if (buttons.some(button => !button))
            return;
        this._system = {item, box, children, buttons: []};
        // GNOME's battery button and the two spacers leave the row; the battery has a tile of its own
        for (const child of children) {
            if (!buttons.includes(child))
                box.remove_child(child);
        }
        const words = [_('Screenshot'), _('Lock'), _('Settings'), _('Power')];
        // Log out is the row's own, GNOME's entry being inside the Power menu. zz1 sets always-show-log-out, without which
        // GNOME takes logging out to be unavailable on a machine with one account
        const logOut = new QuickSettingsItem({style_class: 'icon-button', can_focus: true, accessible_name: _('Log out')});
        logOut.set_child(new St.Icon({icon_name: 'system-log-out-symbolic', style_class: 'popup-menu-icon'}));
        logOut.connect('clicked', () => {
            Main.panel.closeQuickSettings();
            SystemActions.getDefault().activateLogout();
        });
        SystemActions.getDefault().bind_property('can-logout', logOut, 'visible', GObject.BindingFlags.SYNC_CREATE);
        box.insert_child_at_index(logOut, 2);
        this._logOut = logOut;
        buttons.splice(2, 0, logOut);
        words.splice(2, 0, _('Log out'));
        buttons.forEach((button, i) => {
            box.set_child_at_index(button, i);
            const icon = button.get_child();
            button.set_child(null);
            // The icon above its word: five words side by side do not fit
            const content = new St.BoxLayout({style_class: 'amethystora-quick-action-box',
                orientation: Clutter.Orientation.VERTICAL,
                x_align: Clutter.ActorAlign.CENTER, y_align: Clutter.ActorAlign.CENTER});
            if (icon)
                content.add_child(icon);
            content.add_child(new St.Label({text: words[i], x_align: Clutter.ActorAlign.CENTER}));
            button.set_child(content);
            this._system.buttons.push({button, icon, content, expand: button.x_expand});
            button.x_expand = true;
            button.add_style_class_name('amethystora-quick-action');
        });
        item.add_style_class_name('amethystora-quick-actions');
        // As wide as the blocks, whatever the words of the buttons
        item.natural_width = BLOCK_WIDTH;
    }

    _releaseSystem() {
        const system = this._system;
        this._system = null;
        if (!system)
            return;
        for (const {button, icon, content, expand} of system.buttons) {
            if (icon)
                content.remove_child(icon);
            button.set_child(icon);
            content.destroy();
            button.x_expand = expand;
            button.remove_style_class_name('amethystora-quick-action');
        }
        this._logOut.destroy();
        this._logOut = null;
        // GNOME's own order: the battery, a spacer, Screenshot, Settings, a spacer, Lock, Power
        system.children.forEach((child, i) => {
            if (child.get_parent() !== system.box)
                system.box.insert_child_at_index(child, i);
            else
                system.box.set_child_at_index(child, i);
        });
        system.item.remove_style_class_name('amethystora-quick-actions');
        system.item.natural_width_set = false;
    }

    // --- The hint and the Hidden row -------------------------------------------------------------------------------

    _buildHint() {
        this._hint = new St.BoxLayout({style_class: 'amethystora-quick-hint', x_expand: true});
        this._hintLabel = new St.Label({style_class: 'amethystora-quick-hint-label',
            text: _('Arrange: hold a tile and drag it'), x_expand: true, y_align: Clutter.ActorAlign.CENTER});
        this._hint.add_child(this._hintLabel);
        this._done = new St.Button({style_class: 'amethystora-quick-done', label: _('Done'), can_focus: true,
            visible: false, y_align: Clutter.ActorAlign.CENTER});
        this._done.connect('clicked', () => this.arrange(false));
        this._hint.add_child(this._done);
        this._insert(this._hint, 2);
    }

    _buildHiddenRow() {
        this._hiddenRow = new St.BoxLayout({style_class: 'amethystora-quick-hidden',
            orientation: Clutter.Orientation.VERTICAL, x_expand: true});
        this._hiddenRow.add_child(new St.Label({style_class: 'amethystora-quick-heading',
            text: _('Hidden').toLocaleUpperCase()}));
        // Two to a row, as in the grid
        this._hiddenLayout = new Clutter.GridLayout({column_spacing: 10, row_spacing: 10});
        this._hiddenBox = new St.Widget({style_class: 'amethystora-quick-hidden-tiles',
            layout_manager: this._hiddenLayout, x_expand: true});
        this._hiddenRow.add_child(this._hiddenBox);
        // The theme block, while put away, waits in the Hidden row as a tile of its own name
        this._themeStandIn = new QuickToggle({title: _('Theme'), iconName: 'applications-graphics-symbolic',
            can_focus: false, reactive: false, opacity: HIDDEN_OPACITY});
    }

    _placeHidden() {
        this._hiddenBox.get_children().forEach((actor, i) => {
            this._hiddenLayout.child_set_property(this._hiddenBox, actor, 'left-attach', i % 2);
            this._hiddenLayout.child_set_property(this._hiddenBox, actor, 'top-attach', Math.floor(i / 2));
        });
    }

    // Arrange mode came or went: the hint says what to do, and the Hidden row shows or goes
    arrangeChanged(on) {
        this._hintLabel.text = on ? _('Drag a tile to move it') : _('Arrange: hold a tile and drag it');
        this._done.visible = on;
        if (on)
            this._hint.add_style_pseudo_class('active');
        else
            this._hint.remove_style_pseudo_class('active');
        this._layout();
    }

    // --- What arrange mode asks ---------------------------------------------------------------------------------------

    // The tiles in the grid that are shown, in their order
    tiles() {
        return this.grid.get_children().filter(actor => this._known.has(actor) && actor.visible);
    }

    // What arrange mode gives a handle: the tiles, the theme block, and what waits in the Hidden row
    arrangeable() {
        const actors = this.tiles();
        if (this._theme.block.get_parent() === this.grid && this._theme.block.visible)
            actors.push(this._theme.block);
        if (this._hiddenRow.get_parent() === this.grid)
            actors.push(...this._hiddenBox.get_children().filter(actor => actor.visible));
        return actors;
    }

    // Every tile there is, shown or not, and the theme block and its stand-in: what arrange mode may have shrunk
    everyArrangeable() {
        return [...this._known, this._theme.block, this._themeStandIn].filter(Boolean);
    }

    hiddenActor(actor) {
        return actor.get_parent() === this._hiddenBox;
    }

    movable(actor) {
        return this._known.has(actor) && actor.get_parent() === this.grid;
    }

    // The tiles in this order, written to the key: every tile shown, and the ones the key named that are not there now
    // each kept after the one it followed
    commitOrder(actors) {
        const ids = actors.map(actor => this.idOf(actor));
        const before = this._ids('tile-order');
        const result = [...ids];
        let after = null;
        for (const id of before) {
            if (result.includes(id)) {
                after = id;
                continue;
            }
            const at = after === null ? 0 : result.indexOf(after) + 1;
            result.splice(at, 0, id);
            after = id;
        }
        this._settings.set_strv('tile-order', [...new Set(result)].filter(id => ID.test(id)));
    }

    hide(id) {
        if (!ID.test(id))
            return;
        const hidden = this._ids('tile-hidden');
        if (!hidden.includes(id))
            this._settings.set_strv('tile-hidden', [...hidden, id]);
    }

    show(id) {
        const hidden = this._ids('tile-hidden');
        if (hidden.includes(id))
            this._settings.set_strv('tile-hidden', hidden.filter(each => each !== id));
    }

    // --- Open and closed -------------------------------------------------------------------------------------------

    _openChanged(open) {
        if (open) {
            this._subtitles.opened();
            this._battery.opened();
            this._theme.opened();
            this._volume.opened();
        } else {
            this._arrange?.leave();
            this._subtitles.closed();
            this._battery.closed();
            this._theme.closed();
            this._volume.closed();
        }
    }

    // --- For the VM suite ------------------------------------------------------------------------------------------

    tileIds() {
        return this.tiles().map(actor => this.idOf(actor));
    }

    hiddenIds() {
        return this._ids('tile-hidden');
    }

    arrange(on, tile = null) {
        if (on && this.menu?.isOpen)
            this._arrange.enter(tile);
        else if (!on)
            this._arrange?.leave();
        return this._arrange?.active ?? false;
    }

    hideForTest(id) {
        const entry = this._arrange?.handleFor(id);
        if (entry)
            entry.corner.emit('clicked', 1);
        else
            this.hide(id);
        return this.hiddenIds().includes(id);
    }

    moveForTest(id, beforeId) {
        const tiles = this.tiles();
        const moving = tiles.find(actor => this.idOf(actor) === id);
        const target = tiles.find(actor => this.idOf(actor) === beforeId);
        if (!moving || !target || moving === target)
            return false;
        const order = tiles.filter(actor => actor !== moving);
        order.splice(order.indexOf(target), 0, moving);
        this.commitOrder(order);
        return true;
    }

    themeSwatches() {
        return this._theme.swatches();
    }

    pickTheme(slug) {
        return this._theme.pick(slug);
    }

    appRows() {
        return this._volume.rows();
    }

    setAppRowVolume(name, fraction) {
        return this._volume.setRowVolume(name, fraction);
    }

    muteForTest(name) {
        return this._volume.mute(name);
    }

    outputs() {
        return this._volume.outputs();
    }

    pickOutput(description) {
        return this._volume.pickOutput(description);
    }

    batteryRow() {
        return this._battery.limitRow();
    }

    // The battery tile's menu, which the power module adds its explanation to (modules/power/battery.js)
    batteryMenu() {
        return this._battery?.tile.menu ?? null;
    }

    // What the power module's device batteries say for the tile on a computer without a battery of its own: the lowest
    // device, {label, percent}, or null (modules/power/devices.js)
    setOtherBatteries(lowest) {
        this._battery?.setOthers(lowest);
    }

    batterySubtitle() {
        return this._battery.tile.subtitle ?? '';
    }

    toggleBatteryLimit() {
        this._battery.toggleLimit();
        return true;
    }
}
