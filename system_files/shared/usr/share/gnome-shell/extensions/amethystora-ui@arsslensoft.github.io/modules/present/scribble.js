// Scribble (board 38, Super+Alt+D): ink over the whole screen while presenting or in a call. The shell holds the
// pointer and the keyboard while it is up, so the windows under it take no clicks and no keys until it ends ("they
// freeze under the drawing"); Print still opens GNOME's screenshot bar, which keeps the ink in its picture. A pen, a
// highlighter and three colours; a pen's pressure is not read (GJS gets only the first of Clutter's axes). Ctrl+Z takes
// the last stroke back, Esc wipes the ink and ends. Nothing is ever saved. Each stroke is its own small canvas the size of
// its bounds, so a move of the pen repaints that stroke and nothing else.

import Clutter from 'gi://Clutter';
import Gio from 'gi://Gio';
import St from 'gi://St';
import Shell from 'gi://Shell';

import {gettext as _} from 'resource:///org/gnome/shell/extensions/extension.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';

// The board's ink: the pen 4 px and opaque, the highlighter 16 px at .45; three colours that read on any slide
export const COLOURS = ['#e8457a', '#ffd65a', '#4cc3ff'];
const TOOLS = {pen: {width: 4, opacity: 255}, highlighter: {width: 16, opacity: Math.round(0.45 * 255)}};
// The toolbar's distance from the bottom of the primary screen
const BOTTOM = 22;
// The margin a stroke's canvas grows by, and the most points one stroke holds
const GROW = 64;
const MOST_POINTS = 5000;

function parseColour(hex) {
    return [1, 3, 5].map(i => parseInt(hex.slice(i, i + 2), 16) / 255);
}

class Stroke {
    constructor(layer, tool, colour) {
        this.tool = tool;
        this._rgb = parseColour(colour);
        this._points = [];
        this._pad = TOOLS[tool].width * 1.75 / 2 + 2;
        this.actor = new St.DrawingArea({reactive: false, opacity: TOOLS[tool].opacity});
        this.actor.connect('repaint', area => this._repaint(area));
        layer.add_child(this.actor);
    }

    // A point of the stroke
    add(x, y) {
        const width = TOOLS[this.tool].width;
        this._points.push({x, y, width});
        // The bounds grow with the stroke, never shrink while it is drawn, and by a margin at a time, so that most moves
        // only repaint
        const b = this._bounds;
        const p = this._pad;
        if (!b || x - p < b[0] || y - p < b[1] || x + p > b[2] || y + p > b[3]) {
            this._bounds = b
                ? [Math.min(b[0], Math.floor(x - p - GROW)), Math.min(b[1], Math.floor(y - p - GROW)),
                    Math.max(b[2], Math.ceil(x + p + GROW)), Math.max(b[3], Math.ceil(y + p + GROW))]
                : [Math.floor(x - p - GROW), Math.floor(y - p - GROW), Math.ceil(x + p + GROW), Math.ceil(y + p + GROW)];
            const [x1, y1, x2, y2] = this._bounds;
            this.actor.set_position(x1, y1);
            this.actor.set_size(x2 - x1, y2 - y1);
        }
        this.actor.queue_repaint();
    }

    // A stroke this long is ended and the next one starts where it stopped, so that no repaint grows without end
    get full() {
        return this._points.length >= MOST_POINTS;
    }

    get last() {
        return this._points.at(-1);
    }

    _repaint(area) {
        const cr = area.get_context();
        const [surfaceWidth] = area.get_surface_size();
        const scale = surfaceWidth / Math.max(1, this.actor.width);
        cr.scale(scale, scale);
        cr.translate(-this.actor.x, -this.actor.y);
        cr.setSourceRGBA(...this._rgb, 1);
        cr.setLineCap(1);
        cr.setLineJoin(1);
        const points = this._points;
        if (points.length === 1) {
            const [p] = points;
            cr.arc(p.x, p.y, p.width / 2, 0, 2 * Math.PI);
            cr.fill();
        } else if (points.every(p => p.width === points[0].width)) {
            // One path, so that the highlighter is even where it crosses itself
            cr.setLineWidth(points[0].width);
            cr.moveTo(points[0].x, points[0].y);
            for (const p of points.slice(1))
                cr.lineTo(p.x, p.y);
            cr.stroke();
        } else {
            for (let i = 1; i < points.length; i++) {
                cr.setLineWidth((points[i - 1].width + points[i].width) / 2);
                cr.moveTo(points[i - 1].x, points[i - 1].y);
                cr.lineTo(points[i].x, points[i].y);
                cr.stroke();
            }
        }
        cr.$dispose();
    }

    destroy() {
        this.actor.destroy();
    }
}

export class Scribble {
    // module: the present module, which is told when Scribble ends and lists the toolbar among its glass surfaces;
    // iconDir: the extension's own icons
    constructor(module, iconDir) {
        this._module = module;
        this._iconDir = iconDir;
        this._strokes = [];
        this._tool = 'pen';
        this._colour = COLOURS[0];
        this._stroke = null;
    }

    get isOpen() {
        return Boolean(this.layer);
    }

    get strokes() {
        return this._strokes.length;
    }

    get toolbar() {
        return this._toolbar ?? null;
    }

    // Up over everything, the shell holding input
    open() {
        if (this.layer)
            return true;
        this.layer = new St.Widget({
            name: 'amethystoraScribble',
            reactive: true,
            can_focus: true,
            accessible_name: _('Drawing on the screen'),
        });
        this.layer.add_constraint(new Clutter.BindConstraint({source: global.stage, coordinate: Clutter.BindCoordinate.SIZE}));
        this.layer.cursor_type = Clutter.CursorType.CROSSHAIR;
        this._ink = new Clutter.Actor({reactive: false});
        this.layer.add_child(this._ink);
        Main.layoutManager.uiGroup.add_child(this.layer);
        Main.layoutManager.uiGroup.set_child_above_sibling(this.layer, null);

        this._grab = Main.pushModal(this.layer, {actionMode: Shell.ActionMode.POPUP});
        // Another grab kept the input (GNOME 50 has no seat state on a grab to ask): no ink over windows that still
        // take the keys, and none that Esc could not end
        if (global.stage.get_grab_actor() !== this.layer) {
            this.close();
            return false;
        }
        // Never over the lock screen, whether or not the extension stops at a lock
        Main.sessionMode.connectObject('updated', () => {
            if (Main.sessionMode.isLocked)
                this.close();
        }, this);
        this.layer.grab_key_focus();
        this.layer.connectObject(
            'button-press-event', (a, event) => this._press(event),
            'motion-event', (a, event) => this._motion(event),
            'button-release-event', () => this._release(),
            'touch-event', (a, event) => this._touch(event),
            'key-press-event', (a, event) => this._key(event), this);
        this._buildToolbar();
        return true;
    }

    _icon(name) {
        return Gio.FileIcon.new(this._iconDir.get_child('screenshot').get_child(`${name}-symbolic.svg`));
    }

    _buildToolbar() {
        // A press on the bar, between its buttons too, draws nothing (_onToolbar)
        const bar = new St.BoxLayout({style_class: 'amethystora-present-toolbar', reactive: true});
        this._toolButtons = new Map();
        for (const [tool, icon, name] of [['pen', 'pen', _('Pen')], ['highlighter', 'highlight', _('Highlighter')]]) {
            const button = new St.Button({
                style_class: 'amethystora-present-tool',
                child: new St.Icon({gicon: this._icon(icon), icon_size: 16}),
                accessible_name: name,
                toggle_mode: true,
                can_focus: false,
            });
            button.connect('clicked', () => this.setTool(tool));
            this._toolButtons.set(tool, button);
            bar.add_child(button);
        }
        bar.add_child(new St.Widget({style_class: 'amethystora-present-separator', y_align: Clutter.ActorAlign.CENTER}));
        this._colourButtons = new Map();
        COLOURS.forEach((colour, i) => {
            const dot = new St.Widget({style_class: 'amethystora-present-dot', style: `background-color: ${colour};`,
                x_align: Clutter.ActorAlign.CENTER, y_align: Clutter.ActorAlign.CENTER});
            const button = new St.Button({
                style_class: 'amethystora-present-tool',
                child: dot,
                accessible_name: [_('Pink'), _('Yellow'), _('Blue')][i],
                toggle_mode: true,
                can_focus: false,
            });
            button.connect('clicked', () => this.setColour(colour));
            this._colourButtons.set(colour, button);
            bar.add_child(button);
        });
        bar.add_child(new St.Widget({style_class: 'amethystora-present-separator', y_align: Clutter.ActorAlign.CENTER}));
        const clear = new St.Button({
            style_class: 'amethystora-present-clear',
            label: _('Clear · Esc'),
            can_focus: false,
        });
        clear.connect('clicked', () => this.close());
        bar.add_child(clear);
        this._toolbar = bar;
        this.layer.add_child(bar);
        const monitor = Main.layoutManager.primaryMonitor;
        const [, width] = bar.get_preferred_width(-1);
        const [, height] = bar.get_preferred_height(width);
        bar.set_position(Math.round(monitor.x + (monitor.width - width) / 2), monitor.y + monitor.height - BOTTOM - height);
        this._syncButtons();
        this._module.surfacesChanged();
    }

    _syncButtons() {
        for (const [tool, button] of this._toolButtons ?? [])
            button.checked = tool === this._tool;
        for (const [colour, button] of this._colourButtons ?? [])
            button.checked = colour === this._colour;
    }

    setTool(tool) {
        this._tool = tool;
        this._syncButtons();
    }

    setColour(colour) {
        this._colour = colour;
        this._syncButtons();
    }

    // ---- Drawing ----

    // On the toolbar, an event is left to its buttons: Clutter drops the click gestures of a pointer as soon as an actor
    // stops one of its events, so none is stopped there
    _onToolbar(event) {
        return this._toolbar?.contains(global.stage.get_event_actor(event)) ?? false;
    }

    _press(event) {
        if (this._onToolbar(event))
            return Clutter.EVENT_PROPAGATE;
        if (event.get_button() !== Clutter.BUTTON_PRIMARY)
            return Clutter.EVENT_STOP;
        this._begin(event);
        return Clutter.EVENT_STOP;
    }

    _begin(event) {
        this._stroke = new Stroke(this._ink, this._tool, this._colour);
        this._strokes.push(this._stroke);
        const [x, y] = event.get_coords();
        this._stroke.add(x, y);
    }

    _motion(event) {
        if (!this._stroke)
            return Clutter.EVENT_PROPAGATE;
        const [x, y] = event.get_coords();
        if (this._stroke.full) {
            const {x: lx, y: ly} = this._stroke.last;
            this._stroke = new Stroke(this._ink, this._tool, this._colour);
            this._strokes.push(this._stroke);
            this._stroke.add(lx, ly);
        }
        this._stroke.add(x, y);
        return Clutter.EVENT_STOP;
    }

    _release() {
        this._stroke = null;
        return Clutter.EVENT_STOP;
    }

    _touch(event) {
        const type = event.type();
        if (!this._stroke && this._onToolbar(event))
            return Clutter.EVENT_PROPAGATE;
        if (type === Clutter.EventType.TOUCH_BEGIN)
            this._begin(event);
        else if (type === Clutter.EventType.TOUCH_UPDATE)
            this._motion(event);
        else
            this._release();
        return Clutter.EVENT_STOP;
    }

    _key(event) {
        const symbol = event.get_key_symbol();
        const ctrl = (event.get_state() & Clutter.ModifierType.CONTROL_MASK) !== 0;
        if (symbol === Clutter.KEY_Escape) {
            this.close();
        } else if (ctrl && (symbol === Clutter.KEY_z || symbol === Clutter.KEY_Z)) {
            this.undo();
        } else if (!ctrl && [Clutter.KEY_1, Clutter.KEY_2, Clutter.KEY_3].includes(symbol)) {
            this.setColour(COLOURS[symbol - Clutter.KEY_1]);
        } else if (!ctrl && (symbol === Clutter.KEY_p || symbol === Clutter.KEY_P)) {
            this.setTool('pen');
        } else if (!ctrl && (symbol === Clutter.KEY_h || symbol === Clutter.KEY_H)) {
            this.setTool('highlighter');
        }
        return Clutter.EVENT_STOP;
    }

    undo() {
        if (this._stroke)
            return;
        this._strokes.pop()?.destroy();
    }

    // The ink is wiped and the windows get their input back
    close() {
        if (!this.layer)
            return;
        this._stroke = null;
        for (const stroke of this._strokes)
            stroke.destroy();
        this._strokes = [];
        Main.sessionMode.disconnectObject(this);
        if (this._grab)
            Main.popModal(this._grab);
        this._grab = null;
        this.layer.destroy();
        this.layer = null;
        this._ink = null;
        this._toolbar = null;
        this._toolButtons = null;
        this._colourButtons = null;
        this._module.scribbleClosed();
    }
}
