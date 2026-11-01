// Arrange mode: holding a tile of quick settings (or Menu on a focused one) makes the tiles movable. Each tile shrinks a
// little and takes a round button at its corner that puts it away; the tiles put away wait in a Hidden row with a
// button that brings each back. A tile is dragged to its new place, the tiles after it making room as it goes, and the
// keyboard moves the focused one (Ctrl and an arrow), puts it away (Delete) or brings it back (Enter).
//
// The tiles themselves stay GNOME's and are never taken out of GNOME's grid to be dragged: a layer over the grid holds
// one handle per tile, which takes the pointer, the keyboard and the corner button, so that no press reaches a tile
// while arranging (a tap toggles nothing); the tile that moves is drawn by a clone of it until it is dropped. The room
// made for it is a translation of the tiles only, never a relayout per frame; the grid is put in its new order once, on
// the drop, by writing the order the module then lays out.

import Clutter from 'gi://Clutter';
import St from 'gi://St';

import {gettext as _} from 'resource:///org/gnome/shell/extensions/extension.js';
import {adjustAnimationTime} from 'resource:///org/gnome/shell/misc/animationUtils.js';

// A tile in arrange mode, and how long it takes to shrink or move aside
const SCALE = 0.96;
const EASE_MS = 150;
// How far a handle reaches beyond its tile, so that a drop in the gap between two tiles still lands on one
const REACH = 5;
// The corner button sits this far in from the tile's corner
const CORNER_INSET = 2;
// The clone of the tile being dragged
const DRAG_OPACITY = 230;
// The grid's columns (GNOME's quick settings has two)
const COLUMNS = 2;
// The two moves of a tile making room, made once per drag and only aimed again after
const SLIDES = [['translation-x', 'translation_x', 'x'], ['translation-y', 'translation_y', 'y']];

export class Arrange {
    // quick: the module, which knows the tiles, their names and their order, and writes the keys
    constructor(quick) {
        this._quick = quick;
        this._handles = new Map();
        // What took the keyboard before arranging, given back after: no Tab or Space reaches a tile under the handles
        this._focusable = new Map();
        // Its own tracker, so that a tile's other handlers come and go apart from it
        this._focusTracker = {};
    }

    get active() {
        return Boolean(this._layer);
    }

    // keyboardOn: the tile held, or Menu was pressed on, whose handle then takes the keyboard
    enter(keyboardOn = null) {
        if (this._layer)
            return;
        const quick = this._quick;
        quick.menu._activeMenu?.close();
        // Over the grid: bound to GNOME's own layer of toggle menus, which sits exactly over it. An St.Widget, since Tab
        // walks through widgets alone and would never go from one handle to the next through a plain actor
        this._layer = new St.Widget({name: 'amethystora-quick-arrange', reactive: false});
        this._layer.add_constraint(new Clutter.BindConstraint({source: quick.menu._overlay,
            coordinate: Clutter.BindCoordinate.POSITION}));
        this._layer.add_constraint(new Clutter.BindConstraint({source: quick.grid,
            coordinate: Clutter.BindCoordinate.SIZE}));
        quick.menu.actor.add_child(this._layer);
        quick.menu.box.add_style_class_name('amethystora-quick-arranging');
        // Esc ends arranging, not the menu: GNOME's menu manager closes the menu on Esc before any actor of it hears the
        // key, so the menu's close is wrapped on this menu, while arranging, to tell Esc from a click elsewhere
        const menu = quick.menu;
        const close = menu.close;
        menu.close = (...args) => {
            const event = Clutter.get_current_event();
            if (this._layer && event?.type() === Clutter.EventType.KEY_PRESS &&
                event.get_key_symbol() === Clutter.KEY_Escape) {
                this.leave();
                return false;
            }
            return close.apply(menu, args);
        };
        quick.arrangeChanged(true);
        this.refresh();
        // The tile it was started on takes the keyboard, so that the arrows and Delete work at once
        if (keyboardOn)
            this._handles.get(keyboardOn)?.handle.grab_key_focus();
    }

    leave() {
        if (!this._layer)
            return;
        this._endDrag(false);
        // The menu's own close again
        delete this._quick.menu.close;
        // The tile whose handle had the keyboard takes it back, so that a keyboard user stays in the grid
        const focused = [...this._handles.values()].find(entry => entry.handle.has_key_focus())?.actor ?? null;
        for (const {actor} of this._handles.values())
            this._unwatch(actor);
        this._handles.clear();
        this._layer.destroy();
        this._layer = null;
        const quick = this._quick;
        quick.menu.box.remove_style_class_name('amethystora-quick-arranging');
        for (const [widget, could] of this._focusable) {
            widget.disconnectObject(this._focusTracker);
            widget.can_focus = could;
        }
        this._focusable.clear();
        if (quick.menu.isOpen && focused?.mapped && focused.get_parent() === quick.grid) {
            if (focused.can_focus)
                focused.grab_key_focus();
            else
                focused.navigate_focus(null, St.DirectionType.TAB_FORWARD, false);
        }
        // Every tile, the ones GNOME hid meanwhile too, back to its own size and place
        for (const actor of quick.everyArrangeable()) {
            delete actor._amethystoraQuickTo;
            if (actor.visible)
                this._ease(actor, {scale_x: 1, scale_y: 1, translation_x: 0, translation_y: 0});
            else
                actor.set({scale_x: 1, scale_y: 1, translation_x: 0, translation_y: 0});
        }
        quick.arrangeChanged(false);
    }

    // The tiles were laid out again (a key changed, a tile came or went): one handle per tile, where it now is
    refresh() {
        if (!this._layer)
            return;
        // A relayout under a drag (its tile went, another came) ends the drag where it started
        this._endDrag(false);
        const quick = this._quick;
        const wanted = new Map();
        for (const actor of quick.arrangeable())
            wanted.set(actor, quick.hiddenActor(actor));
        for (const [actor, entry] of [...this._handles]) {
            if (!wanted.has(actor) || wanted.get(actor) !== entry.hidden) {
                this._unwatch(actor);
                entry.handle.destroy();
                this._handles.delete(actor);
            }
        }
        for (const [actor, hidden] of wanted) {
            if (!this._handles.has(actor))
                this._handles.set(actor, this._makeHandle(actor, hidden));
            actor.set_pivot_point(0.5, 0.5);
            if (actor.scale_x !== SCALE || actor.translation_x !== 0 || actor.translation_y !== 0)
                this._ease(actor, {scale_x: SCALE, scale_y: SCALE, translation_x: 0, translation_y: 0});
            this._place(actor);
            this._unfocus(actor);
        }
    }

    // A tile and what it holds take no keyboard focus while arranging; the handle over it does
    _unfocus(actor) {
        const walk = widget => {
            if (widget instanceof St.Widget && widget.can_focus && !this._focusable.has(widget)) {
                this._focusable.set(widget, true);
                widget.can_focus = false;
                // A tile destroyed while arranging is forgotten, never written to afterwards
                widget.connectObject('destroy', () => this._focusable.delete(widget), this._focusTracker);
            }
            for (const child of widget.get_children())
                walk(child);
        };
        walk(actor);
    }

    _makeHandle(actor, hidden) {
        const quick = this._quick;
        const id = quick.idOf(actor);
        const title = quick.titleOf(actor);
        const handle = new St.Widget({
            style_class: 'amethystora-quick-handle',
            reactive: true,
            can_focus: true,
            track_hover: true,
            accessible_name: title,
        });
        const corner = new St.Button({
            style_class: 'amethystora-quick-corner',
            can_focus: true,
            accessible_name: (hidden ? _('Show %s') : _('Hide %s')).format(title),
            child: new St.Icon({icon_name: hidden ? 'view-reveal-symbolic' : 'view-conceal-symbolic', icon_size: 14}),
        });
        corner.connect('clicked', () => (hidden ? quick.show(id) : quick.hide(id)));
        handle.add_child(corner);
        handle.connect('key-press-event', (h, event) => this._key(actor, id, hidden, event));

        // Only a tile in the grid moves; the theme block and the tiles put away keep their place
        if (!hidden && quick.movable(actor)) {
            const pan = new Clutter.PanGesture();
            pan.connect('recognize', () => this._beginDrag(actor, pan));
            pan.connect('pan-update', () => this._dragTo(pan));
            pan.connect('end', () => this._drag?.pan === pan && this._endDrag(true));
            pan.connect('cancel', () => this._drag?.pan === pan && this._endDrag(false));
            handle.add_action(pan);
        }
        this._layer.add_child(handle);
        actor.connectObject('notify::allocation', () => this._place(actor), this);
        return {actor, handle, corner, hidden, id};
    }

    _unwatch(actor) {
        actor.disconnectObject(this);
    }

    // Where a tile lies in the grid, without the translation arrange mode gives it
    _box(actor) {
        const grid = this._quick.grid;
        let x = 0, y = 0;
        for (let a = actor; a && a !== grid; a = a.get_parent()) {
            x += a.x;
            y += a.y;
        }
        return {x, y, width: actor.width, height: actor.height};
    }

    _place(actor) {
        const entry = this._handles.get(actor);
        if (!entry || !actor.get_parent())
            return;
        const box = this._box(actor);
        entry.handle.set_position(box.x - REACH, box.y - REACH);
        entry.handle.set_size(box.width + 2 * REACH, box.height + 2 * REACH);
        // The corner button sits at the shrunken tile's top end corner
        const insetX = REACH + CORNER_INSET + box.width * (1 - SCALE) / 2;
        const insetY = REACH + CORNER_INSET + box.height * (1 - SCALE) / 2;
        const rtl = this._layer.get_text_direction() === Clutter.TextDirection.RTL;
        const [, cornerWidth] = entry.corner.get_preferred_width(-1);
        entry.corner.set_position(Math.round(rtl ? insetX : box.width + 2 * REACH - insetX - cornerWidth),
            Math.round(insetY));
    }

    _ease(actor, params) {
        actor.ease({...params, duration: EASE_MS, mode: Clutter.AnimationMode.EASE_OUT_QUAD});
    }

    _key(actor, id, hidden, event) {
        const quick = this._quick;
        const symbol = event.get_key_symbol();
        const ctrl = (event.get_state() & Clutter.ModifierType.CONTROL_MASK) !== 0;
        const rtl = actor.get_text_direction() === Clutter.TextDirection.RTL;
        if (symbol === Clutter.KEY_Escape) {
            this.leave();
            return Clutter.EVENT_STOP;
        }
        if (hidden) {
            if ([Clutter.KEY_Return, Clutter.KEY_KP_Enter, Clutter.KEY_ISO_Enter, Clutter.KEY_space].includes(symbol)) {
                quick.show(id);
                // The keyboard follows the tile back to its place
                this._handles.get(actor)?.handle.grab_key_focus();
                return Clutter.EVENT_STOP;
            }
            return Clutter.EVENT_PROPAGATE;
        }
        if (symbol === Clutter.KEY_Delete || symbol === Clutter.KEY_KP_Delete) {
            const at = quick.tiles().indexOf(actor);
            quick.hide(id);
            // The keyboard goes to the tile that took its place
            const tiles = quick.tiles();
            const next = tiles[Math.min(Math.max(at, 0), tiles.length - 1)] ?? actor;
            this._handles.get(next)?.handle.grab_key_focus();
            return Clutter.EVENT_STOP;
        }
        if (!ctrl || !quick.movable(actor))
            return Clutter.EVENT_PROPAGATE;
        let step = 0;
        if (symbol === Clutter.KEY_Left)
            step = rtl ? 1 : -1;
        else if (symbol === Clutter.KEY_Right)
            step = rtl ? -1 : 1;
        else if (symbol === Clutter.KEY_Up)
            step = -COLUMNS;
        else if (symbol === Clutter.KEY_Down)
            step = COLUMNS;
        if (step === 0)
            return Clutter.EVENT_PROPAGATE;
        const tiles = quick.tiles();
        const from = tiles.indexOf(actor);
        const to = Math.max(0, Math.min(tiles.length - 1, from + step));
        if (from >= 0 && to !== from) {
            const order = tiles.filter(t => t !== actor);
            order.splice(to, 0, actor);
            quick.commitOrder(order);
            // The handle is made again for the tile's new place: the keyboard stays on it
            this._handles.get(actor)?.handle.grab_key_focus();
        }
        return Clutter.EVENT_STOP;
    }

    // --- Dragging ---------------------------------------------------------------------------------------------------

    _beginDrag(actor, pan) {
        const tiles = this._quick.tiles();
        // One tile at a time: a second finger on another tile waits
        if (this._drag || !tiles.includes(actor))
            return;
        // Each place in the grid, as the tiles lie before the drag
        this._drag = {
            actor,
            tiles,
            slots: tiles.map(t => this._box(t)),
            insert: tiles.indexOf(actor),
            pan,
            gone: new Set(),
        };
        // A tile destroyed under the drag (its module stopped before this one) is never touched again
        const drag = this._drag;
        for (const tile of tiles)
            tile.connectObject('destroy', () => drag.gone.add(tile), drag);
        // The corner buttons stay where the tiles were: they go while the tiles make room
        for (const entry of this._handles.values())
            entry.corner.hide();
        const box = this._box(actor);
        const clone = new Clutter.Clone({source: actor, opacity: DRAG_OPACITY, x: box.x, y: box.y,
            width: box.width, height: box.height, pivot_point: actor.pivot_point});
        this._drag.start = this._drag.insert;
        this._drag.box = box;
        // One transition per tile and axis for the whole drag: aiming one again costs a fraction of a new ease
        const duration = adjustAnimationTime(EASE_MS);
        this._drag.slides = new Map();
        for (const tile of tiles) {
            // With animations off a tile simply goes to its place
            if (tile === actor || duration === 0)
                continue;
            this._drag.slides.set(tile, SLIDES.map(([property]) => {
                const slide = new Clutter.PropertyTransition({property_name: property, duration,
                    progress_mode: Clutter.AnimationMode.EASE_OUT_QUAD, remove_on_complete: false});
                tile.add_transition(`amethystora-quick-${property}`, slide);
                return slide;
            }));
        }
        clone.set_scale(SCALE, SCALE);
        this._layer.add_child(clone);
        this._drag.clone = clone;
        const [x, y] = this._layerPoint(pan);
        this._drag.offset = [x - box.x, y - box.y];
        actor.opacity = 0;
    }

    _layerPoint(pan) {
        const point = pan.get_centroid_abs();
        const [, x, y] = this._layer.transform_stage_point(point.x, point.y);
        return [x, y];
    }

    _dragTo(pan) {
        const drag = this._drag;
        if (!drag || drag.pan !== pan)
            return;
        const [x, y] = this._layerPoint(pan);
        // Placed once, moved by translation: nothing is laid out again while the pointer moves
        drag.clone.set_translation(x - drag.offset[0] - drag.box.x, y - drag.offset[1] - drag.box.y, 0);
        // Outside every tile, the tiles go back to their places, and a drop there changes nothing
        const insert = this._insertAt(x, y) ?? drag.start;
        if (insert === drag.insert)
            return;
        drag.insert = insert;
        this._makeRoom();
    }

    // Where the dragged tile would go for a pointer at x, y: before the tile under it, after it on its end half; null
    // outside every tile
    _insertAt(x, y) {
        const drag = this._drag;
        const rtl = this._layer.get_text_direction() === Clutter.TextDirection.RTL;
        const others = drag.tiles.filter(t => t !== drag.actor);
        for (let i = 0; i < drag.tiles.length; i++) {
            const slot = drag.slots[i];
            if (x < slot.x - REACH || x >= slot.x + slot.width + REACH ||
                y < slot.y - REACH || y >= slot.y + slot.height + REACH)
                continue;
            const tile = drag.tiles[i];
            if (tile === drag.actor)
                return drag.insert;
            const endHalf = rtl ? x < slot.x + slot.width / 2 : x >= slot.x + slot.width / 2;
            const index = others.indexOf(tile);
            return endHalf ? index + 1 : index;
        }
        return null;
    }

    // Every other tile slides to its place in the order the drop would make
    _makeRoom() {
        const drag = this._drag;
        const order = this._order(drag);
        for (const tile of drag.tiles) {
            if (tile === drag.actor || drag.gone.has(tile))
                continue;
            const at = order.indexOf(tile);
            if (tile._amethystoraQuickTo === at)
                continue;
            tile._amethystoraQuickTo = at;
            const from = drag.slots[drag.tiles.indexOf(tile)];
            const to = drag.slots[at];
            const slides = drag.slides.get(tile);
            if (!slides) {
                tile.set_translation(to.x - from.x, to.y - from.y, 0);
                continue;
            }
            slides.forEach((slide, axis) => {
                const [, field, coordinate] = SLIDES[axis];
                slide.stop();
                slide.set_from(tile[field]);
                slide.set_to(to[coordinate] - from[coordinate]);
                slide.rewind();
                slide.start();
            });
        }
    }

    _order(drag) {
        const order = drag.tiles.filter(t => t !== drag.actor);
        order.splice(drag.insert, 0, drag.actor);
        return order;
    }

    _endDrag(dropped) {
        const drag = this._drag;
        if (!drag)
            return;
        this._drag = null;
        for (const entry of this._handles.values())
            entry.corner.show();
        const live = drag.tiles.filter(tile => !drag.gone.has(tile));
        for (const tile of live)
            tile.disconnectObject(drag);
        const order = this._order(drag);
        const moved = dropped && drag.gone.size === 0 && order.some((tile, i) => tile !== drag.tiles[i]);
        drag.clone.destroy();
        if (!drag.gone.has(drag.actor))
            drag.actor.opacity = 255;
        if (moved) {
            // The grid takes its new order at once; the translations made room for exactly that
            this._quick.commitOrder(order);
        }
        for (const tile of live) {
            delete tile._amethystoraQuickTo;
            for (const [property] of SLIDES)
                tile.remove_transition(`amethystora-quick-${property}`);
            if (moved) {
                tile.remove_transition('translation-x');
                tile.remove_transition('translation-y');
                tile.set_translation(0, 0, 0);
            } else if (tile.translation_x !== 0 || tile.translation_y !== 0) {
                this._ease(tile, {translation_x: 0, translation_y: 0});
            }
        }
    }

    // --- For the tests ------------------------------------------------------------------------------------------------

    handleFor(id) {
        for (const entry of this._handles.values()) {
            if (entry.id === id)
                return entry;
        }
        return null;
    }

    destroy() {
        this.leave();
    }
}
