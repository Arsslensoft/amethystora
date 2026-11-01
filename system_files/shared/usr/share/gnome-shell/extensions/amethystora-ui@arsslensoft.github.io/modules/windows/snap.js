// Snap layouts, in the classic layout. Dragging a window by its title bar to the top of the screen, or pressing
// Super+Z, shows six layouts on a glass pane: halves, two thirds and a third, thirds, quarters, the middle, and the
// whole screen. The part of a layout under the pointer (or the keyboard's choice) shows on the screen where the window
// will go, and letting go or choosing puts it there. A window snapped this way gets its old size back when it is
// dragged away again. GNOME's own edge tiling stays: the top bar still maximises, the sides still take half.

import Clutter from 'gi://Clutter';
import GLib from 'gi://GLib';
import Meta from 'gi://Meta';
import Mtk from 'gi://Mtk';
import Shell from 'gi://Shell';
import St from 'gi://St';

import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import {gettext as _} from 'resource:///org/gnome/shell/extensions/extension.js';

import {ease} from './util.js';

// Each layout: the tile's columns and rows as weights, its cells as [column, row, columns spanned, rows spanned],
// and each cell's place on the screen as fractions [left, right, top, bottom] of the work area. The fifth draws its
// two outer columns empty; the sixth is the whole screen, which is maximising.
export const LAYOUTS = [
    {cols: [1, 1], rows: [1], cells: [[0, 0, 1, 1], [1, 0, 1, 1]], targets: [[0, 1 / 2, 0, 1], [1 / 2, 1, 0, 1]]},
    {cols: [2, 1], rows: [1], cells: [[0, 0, 1, 1], [1, 0, 1, 1]], targets: [[0, 2 / 3, 0, 1], [2 / 3, 1, 0, 1]]},
    {
        cols: [1, 1, 1], rows: [1], cells: [[0, 0, 1, 1], [1, 0, 1, 1], [2, 0, 1, 1]],
        targets: [[0, 1 / 3, 0, 1], [1 / 3, 2 / 3, 0, 1], [2 / 3, 1, 0, 1]],
    },
    {
        cols: [1, 1], rows: [1, 1], cells: [[0, 0, 1, 1], [1, 0, 1, 1], [0, 1, 1, 1], [1, 1, 1, 1]],
        targets: [[0, 1 / 2, 0, 1 / 2], [1 / 2, 1, 0, 1 / 2], [0, 1 / 2, 1 / 2, 1], [1 / 2, 1, 1 / 2, 1]],
    },
    {cols: [1, 2, 1], rows: [1], cells: [[1, 0, 1, 1]], targets: [[1 / 4, 3 / 4, 0, 1]]},
    {cols: [1], rows: [1], cells: [[0, 0, 1, 1]], targets: [[0, 1, 0, 1]], maximize: true},
];

// The board's measures, in the stylesheet's pixels
const TILE = [64, 42];
const TILE_PADDING = 3;
const CELL_GAP = 3;
// How near the top of the work area the pointer brings the chooser out, and how far below it it takes it away
const REACH = 48;
// The tip about snap layouts is offered after this many windows dragged in a session
const TIP_AFTER = 3;
// The chooser's distance from the top of the work area
const TOP = 22;
// The place shown first: this far inside the window's place, with these corners
const INSET = 12;
const PREVIEW_RADIUS = 14;

const MOVING = [Meta.GrabOp.MOVING, Meta.GrabOp.MOVING_UNCONSTRAINED].filter(op => op !== undefined);
const UNCONSTRAINED = Meta.GrabOp.WINDOW_FLAG_UNCONSTRAINED ?? 0x400;

// Pixels of the stylesheet as pixels of the stage
const scale = () => St.ThemeContext.get_for_stage(global.stage).scale_factor;

// A cell's rect on a monitor: neighbours share an edge and no pixel is lost
export function cellRect(area, [a, b, c, d]) {
    const x1 = area.x + Math.round(area.width * a);
    const x2 = area.x + Math.round(area.width * b);
    const y1 = area.y + Math.round(area.height * c);
    const y2 = area.y + Math.round(area.height * d);
    return {x: x1, y: y1, width: x2 - x1, height: y2 - y1};
}

const sameRect = (a, b) => a && b && a.x === b.x && a.y === b.y && a.width === b.width && a.height === b.height;

export class Snap {
    constructor(module) {
        this._module = module;
        this._ext = module.ext;
    }

    enable() {
        // What a snapped window was before, until it is dragged away or resized another way
        this._before = new WeakMap();
        // The snapped windows this watches, to let go of them all when it stops
        this._watching = new Set();
        this._targets = [];
        this._hovered = -1;
        this._mode = null; // 'drag' or 'keys'
        Main.wm.addKeybinding('snap-chooser', this._module.settings, Meta.KeyBindingFlags.NONE,
            Shell.ActionMode.NORMAL, () => this.open());
        global.display.connectObject(
            'grab-op-begin', (d, window, op) => this._grabBegin(window, op),
            'grab-op-end', (d, window) => this._grabEnd(window), this);
    }

    disable() {
        Main.wm.removeKeybinding('snap-chooser');
        for (const window of this._watching ?? [])
            window.disconnectObject(this);
        this._watching = null;
        global.display.disconnectObject(this);
        this._stopDrag();
        this._closeKeys();
        if (this._applyId)
            GLib.source_remove(this._applyId);
        this._applyId = 0;
        this._pane?.destroy();
        this._pane = null;
        this._preview?.destroy();
        this._preview = null;
        this._targets = [];
        this._module.surfacesChanged();
    }

    get chooserVisible() {
        return !!this._pane?.visible && this._pane.opacity > 0 && this._showing;
    }

    glassSurfaces() {
        return this._pane ? [{actor: this._pane, material: 'glass', radius: 16}] : [];
    }

    // --- the chooser ----------------------------------------------------------------------------------------------

    // Built on first use and kept hidden afterwards
    _build() {
        if (this._pane)
            return;
        this._pane = new St.BoxLayout({
            style_class: 'amethystora-windows-chooser', orientation: Clutter.Orientation.VERTICAL,
            reactive: true, visible: false, opacity: 0,
        });
        Main.layoutManager.uiGroup.add_child(this._pane);
        this._pane.connectObject('key-press-event', (a, event) => this._keyPress(event),
            'destroy', () => {
                this._pane = null;
                this._targets = [];
                // A pane made again starts hidden: one still counted as showing would never be shown
                this._showing = false;
            }, this);
        const rows = [new St.BoxLayout({style_class: 'amethystora-windows-chooser-row'}),
            new St.BoxLayout({style_class: 'amethystora-windows-chooser-row'})];
        rows.forEach(row => this._pane.add_child(row));
        const s = scale();
        LAYOUTS.forEach((layout, index) => {
            const tile = new St.Widget({style_class: 'amethystora-windows-layout'});
            rows[Math.floor(index / 3)].add_child(tile);
            const inner = [TILE[0] - 2 * TILE_PADDING, TILE[1] - 2 * TILE_PADDING];
            const edges = (weights, length) => {
                const total = weights.reduce((sum, w) => sum + w, 0);
                const free = length - CELL_GAP * (weights.length - 1);
                const starts = [];
                let at = 0;
                for (const w of weights) {
                    starts.push([at, at + free * w / total]);
                    at += free * w / total + CELL_GAP;
                }
                return starts;
            };
            const cols = edges(layout.cols, inner[0]);
            const rowsAt = edges(layout.rows, inner[1]);
            layout.cells.forEach(([col, row, spanX, spanY], cell) => {
                const x1 = cols[col][0];
                const x2 = cols[col + spanX - 1][1];
                const y1 = rowsAt[row][0];
                const y2 = rowsAt[row + spanY - 1][1];
                const actor = new St.Widget({
                    style_class: 'amethystora-windows-cell', reactive: true, track_hover: true,
                    x: Math.round((TILE_PADDING + x1) * s), y: Math.round((TILE_PADDING + y1) * s),
                    width: Math.round((x2 - x1) * s), height: Math.round((y2 - y1) * s),
                });
                tile.add_child(actor);
                const target = {actor, layout: index, cell, fraction: layout.targets[cell], maximize: !!layout.maximize};
                const number = this._targets.length;
                this._targets.push(target);
                actor.connectObject('notify::hover', () => {
                    if (actor.hover && this._mode === 'keys')
                        this._setHovered(number);
                }, this);
                const click = new Clutter.ClickGesture();
                click.connect('recognize', () => {
                    if (this._mode === 'keys')
                        this._choose(number);
                });
                actor.add_action(click);
            });
        });
        this._module.surfacesChanged();
    }

    _place(monitorIndex) {
        const area = Main.layoutManager.getWorkAreaForMonitor(monitorIndex);
        const [, width] = this._pane.get_preferred_width(-1);
        this._pane.set_position(Math.round(area.x + (area.width - width) / 2), area.y + Math.round(TOP * scale()));
        this._monitor = monitorIndex;
    }

    _show(monitorIndex) {
        this._build();
        if (this._showing && this._monitor === monitorIndex)
            return;
        this._place(monitorIndex);
        Main.layoutManager.uiGroup.set_child_above_sibling(this._pane, null);
        if (!this._showing) {
            this._showing = true;
            this._pane.reactive = true;
            this._pane.show();
            this._pane.translation_y = -8 * scale();
            ease(this._ext, this._pane, {opacity: 255, translation_y: 0},
                {duration: 160, mode: Clutter.AnimationMode.EASE_OUT_QUAD});
        }
        this._setHovered(-1);
    }

    _hide() {
        if (!this._showing)
            return;
        this._showing = false;
        this._setHovered(-1);
        const pane = this._pane;
        // It takes no click while it fades
        pane.reactive = false;
        ease(this._ext, pane, {opacity: 0}, {
            duration: 120, mode: Clutter.AnimationMode.EASE_OUT_QUAD,
            onComplete: () => {
                if (!this._showing)
                    pane?.hide();
            },
        });
    }

    // The cell the pointer or the keyboard is on: drawn solid, and its place shown on the screen
    _setHovered(number) {
        if (number === this._hovered)
            return;
        this._targets[this._hovered]?.actor.remove_style_pseudo_class('checked');
        this._hovered = number;
        const target = this._targets[number];
        target?.actor.add_style_pseudo_class('checked');
        this._showPreview(target);
    }

    // For tests and screenshots: hover a cell as the pointer would
    hover(layout, cell) {
        const number = this._targets.findIndex(t => t.layout === layout && t.cell === cell);
        if (number >= 0)
            this._setHovered(number);
        return number;
    }

    _targetRect(target) {
        return cellRect(Main.layoutManager.getWorkAreaForMonitor(this._monitor), target.fraction);
    }

    _showPreview(target) {
        if (!target) {
            if (this._preview?.visible) {
                const preview = this._preview;
                ease(this._ext, preview, {opacity: 0}, {
                    duration: 120,
                    onComplete: () => {
                        if (this._hovered < 0)
                            preview.hide();
                    },
                });
            }
            return;
        }
        if (!this._preview) {
            this._preview = new St.DrawingArea({style_class: 'amethystora-windows-preview', opacity: 0});
            this._preview.connectObject('repaint', area => this._paintPreview(area),
                'destroy', () => (this._preview = null), this);
            global.window_group.add_child(this._preview);
        }
        const rect = this._targetRect(target);
        const inset = INSET * scale();
        global.window_group.set_child_above_sibling(this._preview, null);
        this._preview.set_position(rect.x + inset, rect.y + inset);
        // Painted again only when its size changes
        this._preview.set_size(Math.max(1, rect.width - 2 * inset), Math.max(1, rect.height - 2 * inset));
        if (!this._preview.visible || this._preview.opacity < 255) {
            this._preview.show();
            ease(this._ext, this._preview, {opacity: 255}, {duration: 120});
        }
    }

    // A rounded rectangle in the accent, a tenth or so of it filled, the rest a dashed line: St draws no dashes
    _paintPreview(area) {
        const cr = area.get_context();
        const [width, height] = area.get_surface_size();
        const node = area.get_theme_node();
        const colour = node.get_foreground_color();
        const s = scale();
        const r = Math.min(PREVIEW_RADIUS * s, width / 2, height / 2);
        const line = 2 * s;
        const path = (x, y, w, h, radius) => {
            cr.newSubPath();
            cr.arc(x + w - radius, y + radius, radius, -Math.PI / 2, 0);
            cr.arc(x + w - radius, y + h - radius, radius, 0, Math.PI / 2);
            cr.arc(x + radius, y + h - radius, radius, Math.PI / 2, Math.PI);
            cr.arc(x + radius, y + radius, radius, Math.PI, 3 * Math.PI / 2);
            cr.closePath();
        };
        const [red, green, blue] = [colour.red / 255, colour.green / 255, colour.blue / 255];
        path(0, 0, width, height, r);
        cr.setSourceRGBA(red, green, blue, 0.18);
        cr.fill();
        path(line / 2, line / 2, width - line, height - line, Math.max(0, r - line / 2));
        cr.setSourceRGBA(red, green, blue, 0.75);
        cr.setLineWidth(line);
        cr.setDash([6 * s, 4 * s], 0);
        cr.stroke();
        cr.$dispose();
    }

    // --- dragging -------------------------------------------------------------------------------------------------

    _grabBegin(window, op) {
        if (!window || !MOVING.includes(op & ~UNCONSTRAINED))
            return;
        if (window.get_window_type() !== Meta.WindowType.NORMAL || !window.allows_resize())
            return;
        this._giveBackSize(window);
        this._mode = 'drag';
        this._dragged = window;
        const tracker = global.backend.get_cursor_tracker();
        tracker.connectObject('position-invalidated', () => this._dragMoved(), this);
        this._tracker = tracker;
    }

    _dragMoved() {
        const t0 = GLib.get_monotonic_time();
        const [x, y] = global.get_pointer();
        const monitor = global.display.get_monitor_index_for_rect(new Mtk.Rectangle({x, y, width: 1, height: 1}));
        const area = Main.layoutManager.getWorkAreaForMonitor(monitor);
        const reach = REACH * scale();
        if (!this._showing || this._monitor !== monitor) {
            if (y < area.y + reach)
                this._show(monitor);
        } else {
            const [, bottom] = this._paneBottom();
            if (y > bottom + reach) {
                this._hide();
                return;
            }
        }
        if (this._showing)
            this._setHovered(this._targetAt(x, y));
        this._module.note('snapMove', t0);
    }

    _tip() {
        const hints = this._ext.modules?.get('hints');
        if (!hints || hints.wasShown('snap'))
            return;
        this._module.drags = (this._module.drags ?? 0) + 1;
        if (this._module.drags >= TIP_AFTER) {
            const text = _('Tip: drag a window to the top of the screen to pick a layout, or press Super+Z.');
            hints.offer('snap', {text, anchor: 'pointer'});
        }
    }

    _paneBottom() {
        const extents = this._pane.get_transformed_extents();
        return [extents.origin.y, extents.origin.y + extents.size.height];
    }

    _targetAt(x, y) {
        return this._targets.findIndex(({actor}) => {
            const e = actor.get_transformed_extents();
            return x >= e.origin.x && x < e.origin.x + e.size.width && y >= e.origin.y && y < e.origin.y + e.size.height;
        });
    }

    _grabEnd(window) {
        if (this._mode !== 'drag')
            return;
        const target = this._showing ? this._targets[this._hovered] : null;
        const monitor = this._monitor;
        const dragged = this._dragged;
        this._stopDrag();
        // The tip (the hints module), after the third window dragged in a session without a layout picked
        if (!target)
            this._tip();
        if (!target || window !== dragged)
            return;
        // After Mutter has finished the drag itself
        if (this._applyId)
            GLib.source_remove(this._applyId);
        this._applyId = GLib.idle_add(GLib.PRIORITY_DEFAULT, () => {
            this._applyId = 0;
            this.apply(window, target.layout, target.cell, monitor);
            return GLib.SOURCE_REMOVE;
        });
    }

    _stopDrag() {
        this._tracker?.disconnectObject(this);
        this._tracker = null;
        this._dragged = null;
        if (this._mode === 'drag') {
            this._mode = null;
            this._hide();
        }
    }

    // --- Super+Z --------------------------------------------------------------------------------------------------

    // Opens the chooser for the focused window, as a popup the keyboard and the pointer answer; again closes it
    open() {
        if (this._mode === 'keys') {
            this._closeKeys();
            return false;
        }
        if (this._mode === 'drag')
            return false;
        const window = global.display.focus_window;
        if (!window || window.is_fullscreen() || window.get_window_type() !== Meta.WindowType.NORMAL)
            return false;
        this._build();
        this._window = window;
        this._show(window.get_monitor());
        this._grab = Main.pushModal(this._pane, {actionMode: Shell.ActionMode.POPUP});
        this._mode = 'keys';
        this._pane.grab_key_focus();
        this._setHovered(0);
        // A click outside, or the focus moving elsewhere, closes it
        const owner = this._keysOwner = {};
        this._pane.connectObject('button-press-event', (a, event) => {
            const [x, y] = event.get_coords();
            if (this._targetAt(x, y) < 0 && !this._inside(x, y))
                this._closeKeys();
            return Clutter.EVENT_PROPAGATE;
        }, owner);
        global.display.connectObject('notify::focus-window', () => {
            if (global.display.focus_window !== this._window)
                this._closeKeys();
        }, owner);
        window.connectObject('unmanaging', () => this._closeKeys(), owner);
        return true;
    }

    _inside(x, y) {
        const e = this._pane.get_transformed_extents();
        return x >= e.origin.x && x < e.origin.x + e.size.width && y >= e.origin.y && y < e.origin.y + e.size.height;
    }

    _closeKeys() {
        if (this._mode !== 'keys')
            return;
        this._mode = null;
        if (this._grab)
            Main.popModal(this._grab);
        this._grab = null;
        const owner = this._keysOwner;
        this._keysOwner = null;
        this._pane?.disconnectObject(owner);
        global.display.disconnectObject(owner);
        this._window?.disconnectObject(owner);
        this._window = null;
        this._hide();
    }

    _keyPress(event) {
        if (this._mode !== 'keys')
            return Clutter.EVENT_PROPAGATE;
        const key = event.get_key_symbol();
        const rtl = Clutter.get_default_text_direction() === Clutter.TextDirection.RTL;
        const count = this._targets.length;
        switch (key) {
        case Clutter.KEY_Escape:
            this._closeKeys();
            return Clutter.EVENT_STOP;
        case Clutter.KEY_Return:
        case Clutter.KEY_KP_Enter:
        case Clutter.KEY_ISO_Enter:
        case Clutter.KEY_space:
            this._choose(this._hovered);
            return Clutter.EVENT_STOP;
        case Clutter.KEY_Left:
            this._setHovered((this._hovered + (rtl ? 1 : count - 1)) % count);
            return Clutter.EVENT_STOP;
        case Clutter.KEY_Right:
            this._setHovered((this._hovered + (rtl ? count - 1 : 1)) % count);
            return Clutter.EVENT_STOP;
        case Clutter.KEY_Up:
        case Clutter.KEY_Down:
            this._setHovered(this._otherRow(this._hovered));
            return Clutter.EVENT_STOP;
        case Clutter.KEY_z:
        case Clutter.KEY_Z:
            if (event.get_state() & (Clutter.ModifierType.SUPER_MASK | Clutter.ModifierType.MOD4_MASK)) {
                this._closeKeys();
                return Clutter.EVENT_STOP;
            }
        }
        return Clutter.EVENT_STOP;
    }

    // The cell in the other row of tiles nearest below or above the chosen one
    _otherRow(number) {
        const from = this._targets[number];
        if (!from)
            return 0;
        const row = Math.floor(from.layout / 3);
        const centre = t => {
            const e = t.actor.get_transformed_extents();
            return e.origin.x + e.size.width / 2;
        };
        const x = centre(from);
        let best = number;
        let distance = Infinity;
        this._targets.forEach((t, i) => {
            if (Math.floor(t.layout / 3) === row)
                return;
            const d = Math.abs(centre(t) - x);
            if (d < distance) {
                distance = d;
                best = i;
            }
        });
        return best;
    }

    _choose(number) {
        const target = this._targets[number];
        const window = this._window;
        const monitor = this._monitor;
        this._closeKeys();
        if (target && window)
            this.apply(window, target.layout, target.cell, monitor);
    }

    // --- putting the window there ---------------------------------------------------------------------------------

    // The window into a layout's cell on a monitor; the frame rect it had before is kept for when it is dragged away
    apply(window, layout, cell, monitor = window.get_monitor()) {
        const target = LAYOUTS[layout];
        if (!target || !target.targets[cell] || window.is_fullscreen())
            return null;
        const before = window.get_frame_rect();
        const area = Main.layoutManager.getWorkAreaForMonitor(monitor);
        if (target.maximize) {
            window.maximize();
            return cellRect(area, target.targets[cell]);
        }
        const rect = cellRect(area, target.targets[cell]);
        if (window.is_maximized?.() || window.get_maximize_flags?.())
            window.unmaximize();
        if (!window.allows_resize()) {
            const f = window.get_frame_rect();
            window.move_frame(true, Math.round(rect.x + (rect.width - f.width) / 2),
                Math.round(rect.y + (rect.height - f.height) / 2));
            return rect;
        }
        const kept = this._before.get(window);
        this._before.set(window, {width: kept?.width ?? before.width, height: kept?.height ?? before.height, rect});
        // Mutter takes nothing of a request below the window's smallest size, not even the place: the place first
        window.move_frame(true, rect.x, rect.y);
        window.move_resize_frame(true, rect.x, rect.y, rect.width, rect.height);
        // Resized any other way, the window forgets the size it had (watched once, however often it is snapped)
        window.disconnectObject(this);
        this._watching.add(window);
        window.connectObject('size-changed', () => {
            const memory = this._before.get(window);
            if (!memory)
                return;
            const now = window.get_frame_rect();
            if (!sameRect(now, memory.rect) && !this._settling(window))
                this._forget(window);
        }, 'unmanaging', () => this._forget(window), this);
        this._settleUntil = GLib.get_monotonic_time() + 1000 * 1000;
        this._settlingWindow = window;
        return rect;
    }

    // Mutter may pass through other sizes on the way to the cell, for a second at most
    _settling(window) {
        return window === this._settlingWindow && GLib.get_monotonic_time() < this._settleUntil;
    }

    _forget(window) {
        this._before.delete(window);
        window.disconnectObject(this);
        this._watching?.delete(window);
    }

    // Dragged away from its cell: the old size back, the pointer at the same place across the window, since Mutter
    // keeps the point it holds a window by as a share of its width and height
    _giveBackSize(window) {
        const memory = this._before.get(window);
        if (!memory)
            return;
        this._forget(window);
        const now = window.get_frame_rect();
        if (!sameRect(now, memory.rect))
            return;
        const [x, y] = global.get_pointer();
        const fx = now.width ? (x - now.x) / now.width : 0.5;
        const fy = now.height ? (y - now.y) / now.height : 0;
        window.move_resize_frame(true, Math.round(x - fx * memory.width), Math.round(y - fy * memory.height),
            memory.width, memory.height);
    }

    // For tests: the size a snapped window will get back
    remembered(window) {
        const memory = this._before.get(window);
        return memory ? {width: memory.width, height: memory.height, rect: memory.rect} : null;
    }
}
