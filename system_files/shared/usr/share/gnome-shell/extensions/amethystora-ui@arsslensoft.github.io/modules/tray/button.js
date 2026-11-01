// A tray item's button in the top bar: its icon, with an overlay as an emblem in the corner, and its label if it
// has one. A right click opens the item's menu at once. A left click opens it once the double-click time has passed
// without a second click, since a double click asks the program for its window; an item known to have no window to
// show opens its menu at once. A middle click is the item's secondary action, and scrolling goes to the program in
// whole wheel steps. A touch opens the menu.

import Clutter from 'gi://Clutter';
import GLib from 'gi://GLib';
import GObject from 'gi://GObject';
import Gio from 'gi://Gio';
import St from 'gi://St';

import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import * as PanelMenu from 'resource:///org/gnome/shell/ui/panelMenu.js';

import {NO_MENU} from './dbus.js';
import {RemoteMenu} from './menu.js';

// The panel's own icon size, in logical pixels, and the overlay's, in KDE's proportion to it
export const ICON_SIZE = 16;
const OVERLAY_SIZE = Math.floor(ICON_SIZE / 1.6);
const MISSING_ICON = 'image-loading-symbolic';
// One wheel step, as the programs' side of the protocol counts it (KDE's tray and Qt: 120 a step, positive away
// from the user, that is up or left)
const WHEEL_STEP = 120;

export const ItemButton = GObject.registerClass(
class ItemButton extends PanelMenu.Button {
    /**
     * @param {import('./item.js').Item} item - the tray item
     * @param {import('./icons.js').IconFinder} icons - finds the item's pictures
     */
    _init(item, icons) {
        super._init(0.5, '', false);
        this.add_style_class_name('amethystora-tray-button');
        this._item = item;
        this._icons = icons;
        this._remote = null;
        this._menuTimer = 0;
        this._lastPress = null;
        this._iconGen = 0;
        this._iconsDirty = true;
        this._scrolled = {horizontal: 0, vertical: 0};

        const box = new St.BoxLayout({style_class: 'amethystora-tray-box', y_align: Clutter.ActorAlign.CENTER});
        this._stack = new St.Widget({layout_manager: new Clutter.BinLayout(), y_align: Clutter.ActorAlign.CENTER});
        this._icon = new St.Icon({
            style_class: 'system-status-icon amethystora-tray-icon',
            icon_size: ICON_SIZE,
            fallback_icon_name: MISSING_ICON,
        });
        this._overlay = new St.Icon({
            style_class: 'amethystora-tray-overlay',
            icon_size: OVERLAY_SIZE,
            x_expand: true,
            y_expand: true,
            x_align: Clutter.ActorAlign.END,
            y_align: Clutter.ActorAlign.END,
            visible: false,
        });
        this._wide = null;
        this._stack.add_child(this._icon);
        this._stack.add_child(this._overlay);
        this._label = new St.Label({
            style_class: 'amethystora-tray-label',
            y_align: Clutter.ActorAlign.CENTER,
            visible: false,
        });
        box.add_child(this._stack);
        box.add_child(this._label);
        this.add_child(box);

        // Every button, on its press, and in the capture phase so that the panel button's own toggle never runs
        const press = new Clutter.ClickGesture({recognize_on_press: true});
        press.connect('recognize', gesture => this._pressed(gesture));
        this.add_action_full('amethystora-tray-press', Clutter.EventPhase.CAPTURE, press);

        // Pictures are found only while the button is on screen
        this.connect('notify::mapped', () => {
            if (this.mapped && this._iconsDirty)
                this._loadIcons();
        });
        this.connect('resource-scale-changed', () => this._item.reloadIcons());
        item.connectObject('changed', (i, groups) => this._changed(groups), this);
        this.connect('destroy', () => this._destroyed());

        this._shown = !item.passive;
        this.visible = this._shown;
        this._syncLabel();
        this._syncName();
        this._syncMenu();
    }

    _destroyed() {
        if (this._menuTimer)
            GLib.source_remove(this._menuTimer);
        this._menuTimer = 0;
        this._remote?.destroy();
        this._remote = null;
        this._iconGen++;
    }

    _changed(groups) {
        if (groups.has('status'))
            this._syncVisible();
        if (groups.has('owner') || groups.has('menu')) {
            // A new program behind the item, or another menu: it is read again from the start
            if (groups.has('owner'))
                this._dropMenu();
            this._syncMenu();
        }
        if (['icon', 'attention', 'overlay', 'status', 'owner'].some(group => groups.has(group))) {
            this._iconsDirty = true;
            if (this.mapped)
                this._loadIcons();
        }
        if (groups.has('label'))
            this._syncLabel();
        this._syncName();
    }

    _syncVisible() {
        const shown = !this._item.passive;
        if (shown === this._shown)
            return;
        this._shown = shown;
        // Hiding the button closes its menu
        this.visible = shown;
        this._syncMenu();
        this._item.checkAlive();
    }

    // The menu is read only while the button is shown
    _syncMenu() {
        const path = this._item.props.Menu;
        const wanted = this._shown && typeof path === 'string' && path !== NO_MENU &&
            GLib.Variant.is_object_path(path) ? path : null;
        if (this._remote?.path === wanted)
            return;
        this._dropMenu();
        if (wanted)
            this._remote = new RemoteMenu(this._item, wanted, this.menu);
    }

    _dropMenu() {
        if (!this._remote)
            return;
        this.menu.close();
        this._remote.destroy();
        this._remote = null;
    }

    _syncLabel() {
        const text = this._item.props.XAyatanaLabel;
        this._label.text = typeof text === 'string' ? text : '';
        this._label.visible = this._label.text !== '';
    }

    // What a screen reader says: the item's own description of its icon, its label, its app, its title
    _syncName() {
        const {props} = this._item;
        const described = this._item.needsAttention ? props.AttentionAccessibleDesc : props.IconAccessibleDesc;
        const name = [described, props.XAyatanaLabel, this._item.appName, props.Title]
            .find(text => typeof text === 'string' && text !== '');
        this.accessible_name = name ?? '';
    }

    async _loadIcons() {
        this._iconsDirty = false;
        const gen = ++this._iconGen;
        const look = {
            size: ICON_SIZE,
            scale: St.ThemeContext.get_for_stage(global.stage).scale_factor,
            resourceScale: this.get_resource_scale(),
            style: this._icon.get_theme_node().get_icon_style(),
            rtl: Clutter.get_default_text_direction() === Clutter.TextDirection.RTL,
        };
        let main = null;
        let overlay = null;
        try {
            // An item that needs attention without an attention icon keeps its own
            if (this._item.needsAttention)
                main = await this._icons.find(this._item, 'Attention', look);
            main ??= await this._icons.find(this._item, '', look);
            overlay = await this._icons.find(this._item, 'Overlay', {...look, size: OVERLAY_SIZE});
        } catch (e) {
            console.debug(`Amethystora UI: a tray icon could not be loaded: ${e.message}`);
        }
        if (gen !== this._iconGen)
            return;

        this._wide?.destroy();
        this._wide = null;
        if (main?.wide) {
            // A wide picture, such as a graph, at the icon's height and its own width
            const height = ICON_SIZE * look.scale;
            this._wide = new St.Widget({
                content: main.wide,
                content_gravity: Clutter.ContentGravity.RESIZE_ASPECT,
                width: Math.round(height * main.width / main.height),
                height,
                y_align: Clutter.ActorAlign.CENTER,
            });
            this._stack.insert_child_below(this._wide, this._overlay);
            this._icon.hide();
        } else {
            this._icon.gicon = main?.gicon ?? new Gio.ThemedIcon({name: MISSING_ICON});
            this._icon.show();
        }
        this._overlay.gicon = overlay?.gicon ?? null;
        this._overlay.visible = Boolean(overlay?.gicon);
    }

    _pressed(gesture) {
        const event = gesture.get_point_event(0);
        const time = event.get_time();
        const [x, y] = event.get_coords();
        // A left click's menu, waiting to see whether a double click comes, gives way to any other press
        if (event.type() === Clutter.EventType.TOUCH_BEGIN || gesture.get_button() !== Clutter.BUTTON_PRIMARY) {
            this._cancelMenuTimer();
            this._lastPress = null;
        }

        if (event.type() === Clutter.EventType.TOUCH_BEGIN) {
            this._toggleMenu();
            return;
        }
        switch (gesture.get_button()) {
        case Clutter.BUTTON_SECONDARY:
            this._toggleMenu();
            break;
        case Clutter.BUTTON_MIDDLE:
            Main.panel.menuManager.activeMenu?.close();
            this._item.secondaryActivate(x, y, time);
            break;
        case Clutter.BUTTON_PRIMARY:
            this._primary(x, y, time);
            break;
        }
    }

    _primary(x, y, time) {
        const settings = global.stage.context.get_settings();
        const last = this._lastPress;
        if (last && time - last.time <= settings.double_click_time &&
            Math.hypot(x - last.x, y - last.y) <= settings.double_click_distance) {
            // A double click: the program's window, and no menu
            this._lastPress = null;
            this._cancelMenuTimer();
            if (this._item.methods.Activate !== false)
                this._item.activate(x, y, time);
            return;
        }
        this._lastPress = {time, x, y};

        if (this.menu.isEmpty())
            return;
        this._cancelMenuTimer();
        if (this._item.methods.Activate === false) {
            this._toggleMenu();
            return;
        }
        this._menuTimer = GLib.timeout_add(GLib.PRIORITY_DEFAULT, settings.double_click_time, () => {
            this._menuTimer = 0;
            this._toggleMenu();
            return GLib.SOURCE_REMOVE;
        });
    }

    _cancelMenuTimer() {
        if (this._menuTimer)
            GLib.source_remove(this._menuTimer);
        this._menuTimer = 0;
    }

    _toggleMenu() {
        if (this.menu.isOpen || !this.menu.isEmpty())
            this.menu.toggle();
    }

    // Every scroll over the button is the item's, so that a wheel turned here does nothing behind it too. The twin
    // of an emulated event is the one counted.
    vfunc_scroll_event(event) {
        if (event.get_flags() & Clutter.EventFlags.FLAG_POINTER_EMULATED)
            return Clutter.EVENT_STOP;

        let dx = 0, dy = 0;
        switch (event.get_scroll_direction()) {
        case Clutter.ScrollDirection.UP:
            dy = -1;
            break;
        case Clutter.ScrollDirection.DOWN:
            dy = 1;
            break;
        case Clutter.ScrollDirection.LEFT:
            dx = -1;
            break;
        case Clutter.ScrollDirection.RIGHT:
            dx = 1;
            break;
        case Clutter.ScrollDirection.SMOOTH:
            [dx, dy] = event.get_scroll_delta();
            break;
        }
        this._scroll('horizontal', dx);
        this._scroll('vertical', dy);
        // A touchpad that lets go starts the count again
        if (event.get_scroll_finish_flags() !== Clutter.ScrollFinishFlags.NONE)
            this._scrolled = {horizontal: 0, vertical: 0};
        return Clutter.EVENT_STOP;
    }

    // Clutter counts a wheel step as 1, positive down or right; the program hears whole steps, its own way round
    _scroll(orientation, delta) {
        if (!delta)
            return;
        this._scrolled[orientation] += delta;
        const steps = Math.trunc(this._scrolled[orientation]);
        if (!steps)
            return;
        this._scrolled[orientation] -= steps;
        this._item.scroll(-steps * WHEEL_STEP, orientation);
    }
});
