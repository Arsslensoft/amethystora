// An app's windows fanned out over its menu (spec 1.8): a right click or a long press on an app with windows. The
// screen dims but for the dock, GNOME's menu for the app opens from the icon (without its list of windows, which the
// fan stands for), the icon lifts, and the windows rise out of it into an arc above the menu, the most recent in front
// with a ring. A scroll or Left and Right steps the ring; a click or Space brings that window forward.
//
// GNOME's menus grab their own actor, which would close the menu at a click on a window; the fan therefore holds a grab
// of its own, on a layer that carries the veil, the windows and its own copy of the app's menu.

import Clutter from 'gi://Clutter';
import GLib from 'gi://GLib';
import Shell from 'gi://Shell';
import St from 'gi://St';

import * as AppMenu from 'resource:///org/gnome/shell/ui/appMenu.js';
import * as BoxPointer from 'resource:///org/gnome/shell/ui/boxpointer.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';

import {titleLabel, windowPicture} from './previews.js';
import {EASE, scaleFactor} from './util.js';

const TILE = {width: 180, height: 130, title: 24};
const SPREAD = 106;
const STEP_NEARER = 40;
const TURN = 4;
const SHOWN_EACH_SIDE = 2;
const CLEAR_OF_MENU = 20;
const RISE = {duration: 340, stagger: 35, scale: 0.3};
const SLIDE = 200;
const FOLD = 200;
const VEIL = 200;
// The icon lifted as Lift lifts it, at a 48 px icon
const LIFT = {scale: 1.22, away: 10};

export class Fan {
    constructor(dock) {
        this._dock = dock;
        this.open = false;
        this.ringed = 0;
        this.windows = [];
    }

    // Opens the fan for an app item of a dock; false if the app has no window there
    show(dash, state) {
        this.close(true);
        const windows = dash.windowsOf(state.key);
        if (!windows.length || !(state.key instanceof Shell.App))
            return false;
        const monitor = Main.layoutManager.monitors[dash.monitorIndex];
        if (!monitor)
            return false;
        this.open = true;
        this.windows = windows;
        this.ringed = 0;
        this._dash = dash;
        this._state = state;
        const level = this._dock.motionLevel();
        this._level = level;
        const sf = scaleFactor();

        const layer = new St.Widget({
            reactive: true,
            x: monitor.x,
            y: monitor.y,
            width: monitor.width,
            height: monitor.height,
            layout_manager: new Clutter.FixedLayout(),
        });
        this._layer = layer;
        Main.layoutManager.uiGroup.add_child(layer);
        Main.layoutManager.uiGroup.set_child_above_sibling(layer, null);

        // The veil round the dock's strip, so that the dock stays as it is
        const strip = dash.stripRect();
        const local = {x: strip.x - monitor.x, y: strip.y - monitor.y, width: strip.width, height: strip.height};
        const pieces = [
            [0, 0, monitor.width, local.y],
            [0, local.y + local.height, monitor.width, monitor.height - local.y - local.height],
            [0, local.y, local.x, local.height],
            [local.x + local.width, local.y, monitor.width - local.x - local.width, local.height],
        ];
        this._veil = [];
        for (const [x, y, width, height] of pieces) {
            if (width <= 0 || height <= 0)
                continue;
            const piece = new St.Widget({style_class: 'amethystora-dock-veil', x, y, width, height, opacity: 0});
            layer.add_child(piece);
            piece.ease({opacity: 255, duration: level === 'none' ? 0 : VEIL, mode: EASE.out});
            this._veil.push(piece);
        }

        // The icon lifts
        const icon = dash.visualOf(state.item);
        this._icon = icon;
        if (icon) {
            icon._amethystoraBusy = true;
            const edge = dash.edge;
            const per = dash.iconSize / 48 * sf;
            icon.ease({
                scale_x: LIFT.scale,
                scale_y: LIFT.scale,
                [edge.short === 'y' ? 'translation_y' : 'translation_x']: edge.away * LIFT.away * per,
                [edge.long === 'x' ? 'translation_x' : 'translation_y']: 0,
                duration: level === 'none' ? 0 : SLIDE,
                mode: EASE.out,
            });
        }

        // GNOME's menu for the app, with the dock's wording, without its list of windows
        const menu = new AppMenu.AppMenu(state.button, dash.edge.side,
            {favoritesSection: true, showSingleWindows: true});
        this._menu = menu;
        menu.setApp(state.key);
        menu._windowSection?.actor.hide();
        menu._openWindowsHeader?.hide();
        layer.add_child(menu.actor);
        menu.connect('activate', () => this.close());
        menu.connect('open-state-changed', (m, open) => {
            if (!open)
                this.close();
        });
        menu.open(BoxPointer.PopupAnimation.FULL);
        menu.actor.navigate_focus(null, St.DirectionType.TAB_FORWARD, false);

        this._grab = Main.pushModal(layer, {actionMode: Shell.ActionMode.POPUP});
        layer.connect('captured-event', (actor, event) => this._onEvent(event));

        // The windows, laid out once the menu has its place
        this._tiles = windows.map(window => this._tile(window, state.name()));
        for (const tile of this._tiles) {
            tile.opacity = 0;
            layer.add_child(tile);
        }
        // Once the menu has its place the windows rise; if it settles somewhere else they follow it
        let risen = false;
        menu.actor.connect('notify::allocation', () => {
            if (!this.open || menu.actor.width < 1)
                return;
            const box = menu.actor.get_allocation_box();
            const key = `${box.x1},${box.y1},${box.x2},${box.y2}`;
            if (key === this._menuBox)
                return;
            this._menuBox = key;
            if (risen) {
                this._layout(true);
            } else {
                risen = true;
                this._rise();
            }
        });
        this._dock.glassChanged();
        return true;
    }

    _tile(window, fallback) {
        const tile = new St.Button({style_class: 'amethystora-dock-fan-tile', reactive: true, can_focus: true});
        const box = new St.BoxLayout({orientation: Clutter.Orientation.VERTICAL, x_expand: true});
        const title = new St.Bin({style_class: 'amethystora-dock-fan-title', x_expand: true});
        title.child = titleLabel(window, 'amethystora-dock-fan-name', fallback);
        box.add_child(title);
        box.add_child(windowPicture(window, TILE.width - 2, TILE.height - TILE.title - 2));
        tile.child = box;
        tile.accessible_name = window.title || fallback;
        tile.connect('clicked', () => this._choose(window));
        tile.set_pivot_point(0.5, 0.5);
        window.connectObject('unmanaged', () => {
            const at = this.windows.indexOf(window);
            if (at < 0)
                return;
            this.windows.splice(at, 1);
            this._tiles.splice(at, 1)[0]?.destroy();
            if (!this.windows.length)
                this.close();
            else
                this._layout(true);
        }, this);
        return tile;
    }

    // Where each tile stands for the ringed one: in front over the icon, the others either side, nearer the edge and
    // turned a little more each step out
    _places() {
        const sf = scaleFactor();
        const dash = this._dash;
        const menu = this._menu.actor;
        const monitor = Main.layoutManager.monitors[dash.monitorIndex];
        // Where the menu stands once it has opened: its allocation in the layer, which covers the screen (it still
        // slides in when this is read)
        const box = menu.get_allocation_box();
        const [mx, my, mw] = [monitor.x + box.x1, monitor.y + box.y1, box.get_width()];
        const icon = this._state.restCentre();
        const width = TILE.width * sf;
        const height = TILE.height * sf;
        return this.windows.map((window, i) => {
            const k = i - this.ringed;
            const step = Math.abs(k);
            let x, y, angle = TURN * k;
            if (dash.edgeName === 'bottom') {
                x = icon - width / 2 + k * SPREAD * sf;
                y = my - CLEAR_OF_MENU * sf - height + step * STEP_NEARER * sf;
            } else {
                // Turned through 90 degrees: the fan runs along the row, away from the edge
                y = icon - height / 2 + k * SPREAD * sf;
                const left = dash.edgeName === 'left';
                const out = step * STEP_NEARER * sf;
                x = left ? mx + mw + CLEAR_OF_MENU * sf - out : mx - CLEAR_OF_MENU * sf - width + out;
                angle = left ? -TURN * k : TURN * k;
            }
            return {
                x: x - monitor.x,
                y: y - monitor.y,
                angle,
                shown: step <= SHOWN_EACH_SIDE,
                ringed: k === 0,
                step,
            };
        });
    }

    _rise() {
        if (!this.open)
            return;
        const places = this._places();
        const sf = scaleFactor();
        const [ix, iy] = this._state.button.get_transformed_position();
        const [iw, ih] = this._state.button.get_transformed_size();
        const monitor = Main.layoutManager.monitors[this._dash.monitorIndex];
        const from = {
            x: ix + iw / 2 - monitor.x - TILE.width * sf / 2,
            y: iy + ih / 2 - monitor.y - TILE.height * sf / 2,
        };
        this._tiles.forEach((tile, i) => {
            const place = places[i];
            tile.set_size(TILE.width * sf, TILE.height * sf);
            this._ring(tile, place.ringed);
            const to = {
                x: place.x, y: place.y, rotation_angle_z: place.angle, scale_x: 1, scale_y: 1,
                opacity: place.shown ? 255 : 0,
            };
            if (this._level === 'none') {
                tile.set(to);
            } else if (this._level === 'reduced') {
                tile.set({...to, opacity: 0});
                tile.ease({opacity: to.opacity, duration: 150, mode: EASE.out});
            } else {
                tile.set({
                    x: from.x, y: from.y, scale_x: RISE.scale, scale_y: RISE.scale, opacity: 0,
                    rotation_angle_z: 0,
                });
                tile.ease({...to, duration: RISE.duration, delay: RISE.stagger * place.step, mode: EASE.back});
            }
            tile.reactive = place.shown;
        });
    }

    _layout(animate) {
        const places = this._places();
        this._tiles.forEach((tile, i) => {
            const place = places[i];
            this._ring(tile, place.ringed);
            const to = {x: place.x, y: place.y, rotation_angle_z: place.angle, opacity: place.shown ? 255 : 0};
            if (!animate || this._level === 'none')
                tile.set(to);
            else if (this._level === 'reduced')
                tile.set(to);
            else
                tile.ease({...to, duration: SLIDE, mode: EASE.quart});
            tile.reactive = place.shown;
        });
    }

    _ring(tile, on) {
        if (on)
            tile.add_style_class_name('amethystora-dock-fan-ringed');
        else
            tile.remove_style_class_name('amethystora-dock-fan-ringed');
    }

    step(by) {
        if (!this.open || !this.windows.length)
            return;
        const next = Math.clamp(this.ringed + by, 0, this.windows.length - 1);
        if (next === this.ringed)
            return;
        this.ringed = next;
        this._layout(true);
    }

    _choose(window) {
        this.close();
        Main.activateWindow(window);
    }

    _onEvent(event) {
        const type = event.type();
        if (type === Clutter.EventType.KEY_PRESS) {
            const key = event.get_key_symbol();
            if (key === Clutter.KEY_Escape) {
                this.close();
                return Clutter.EVENT_STOP;
            }
            if (key === Clutter.KEY_Left || key === Clutter.KEY_Right) {
                const rtl = Clutter.get_default_text_direction() === Clutter.TextDirection.RTL;
                const forward = (key === Clutter.KEY_Right) !== rtl;
                this.step(forward ? 1 : -1);
                return Clutter.EVENT_STOP;
            }
            if (key === Clutter.KEY_space) {
                const window = this.windows[this.ringed];
                if (window)
                    this._choose(window);
                return Clutter.EVENT_STOP;
            }
            return Clutter.EVENT_PROPAGATE;
        }
        if (type === Clutter.EventType.SCROLL) {
            const direction = event.get_scroll_direction();
            if (direction === Clutter.ScrollDirection.UP || direction === Clutter.ScrollDirection.LEFT) {
                this.step(-1);
            } else if (direction === Clutter.ScrollDirection.DOWN || direction === Clutter.ScrollDirection.RIGHT) {
                this.step(1);
            } else if (direction === Clutter.ScrollDirection.SMOOTH) {
                const [dx, dy] = event.get_scroll_delta();
                const delta = Math.abs(dx) > Math.abs(dy) ? dx : dy;
                if (Math.abs(delta) >= 1)
                    this.step(delta > 0 ? 1 : -1);
            }
            return Clutter.EVENT_STOP;
        }
        if (type === Clutter.EventType.BUTTON_PRESS || type === Clutter.EventType.TOUCH_BEGIN) {
            // A press on the menu or a window is theirs; anywhere else closes the fan
            const source = global.stage.get_event_actor(event);
            const mine = this._menu?.actor.contains(source) || this._tiles.some(tile => tile.contains(source));
            if (!mine) {
                this.close();
                return Clutter.EVENT_STOP;
            }
        }
        return Clutter.EVENT_PROPAGATE;
    }

    close(now = false) {
        if (!this.open)
            return;
        this.open = false;
        for (const window of this.windows)
            window.disconnectObject(this);
        if (this._grab) {
            Main.popModal(this._grab);
            this._grab = null;
        }
        const menu = this._menu;
        this._menu = null;
        if (menu) {
            menu.close(BoxPointer.PopupAnimation.NONE);
            // A click on one of its lines ends here: GNOME goes on to close this menu and to end the press on that
            // line once its signal returns, so the menu outlives the event, and the layer, which may go at once
            menu.actor.get_parent()?.remove_child(menu.actor);
            GLib.idle_add(GLib.PRIORITY_HIGH, () => {
                menu.destroy();
                return GLib.SOURCE_REMOVE;
            });
        }
        const icon = this._icon;
        const dash = this._dash;
        this._icon = null;
        if (icon) {
            icon.ease({
                scale_x: 1, scale_y: 1, translation_x: 0, translation_y: 0,
                duration: now || this._level === 'none' ? 0 : FOLD,
                mode: EASE.out,
                onComplete: () => {
                    icon._amethystoraBusy = false;
                    dash?.swell?.sync();
                },
            });
        }
        const layer = this._layer;
        this._layer = null;
        const tiles = this._tiles ?? [];
        this._tiles = [];
        this.windows = [];
        if (layer) {
            layer.reactive = false;
            if (now || this._level !== 'full') {
                layer.destroy();
            } else {
                const [ix, iy] = this._state.button.get_transformed_position();
                const [iw, ih] = this._state.button.get_transformed_size();
                const monitor = Main.layoutManager.monitors[this._dash.monitorIndex] ?? {x: 0, y: 0};
                const sf = scaleFactor();
                for (const tile of tiles) {
                    tile.ease({
                        x: ix + iw / 2 - monitor.x - TILE.width * sf / 2,
                        y: iy + ih / 2 - monitor.y - TILE.height * sf / 2,
                        scale_x: RISE.scale, scale_y: RISE.scale, opacity: 0, rotation_angle_z: 0,
                        duration: FOLD, mode: EASE.out,
                    });
                }
                const veil = this._veil ?? [];
                if (!veil.length)
                    layer.destroy();
                veil.forEach((piece, i) => piece.ease({
                    opacity: 0, duration: FOLD, mode: EASE.out,
                    onComplete: i === 0 ? () => layer.destroy() : undefined,
                }));
            }
        }
        this._veil = [];
        this._dash = null;
        this._state = null;
        this._dock.glassChanged();
    }

    // The tiles for the glass
    get surfaces() {
        return this.open ? this._tiles : [];
    }
}
