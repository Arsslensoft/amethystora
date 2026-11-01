// The pen's tools: six of them around the pen's tip (the ring) or in a column at the screen's edge (the strip), opened
// by the pen's side button once GNOME's tablet settings make that button send the ring's shortcut (ame desktop touch
// pen-button). In the ring, moving the pen from the centre toward a tool lights it; a touch of the pen, or carrying on
// past the ring's edge (a flick), runs it. A touch in the middle, Escape or a click outside closes it. Everything is
// made when it opens and taken away when it closes.
//
//   scribble     drawing on the screen, when a module offers it (round 4's present mode)
//   area         GNOME's screenshot UI on a selection
//   note         Notes' quick note
//   text         the screenshot UI on a selection whose capture copies the text in it (Extract Text, through Gradia)
//   right-click  a right click where the ring opened
//   back         the back button (8) where the ring opened

import Clutter from 'gi://Clutter';
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import Shell from 'gi://Shell';
import St from 'gi://St';

import {gettext as _} from 'resource:///org/gnome/shell/extensions/extension.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';

import {lineIcon} from '../access/lines.js';
import {gradiaInstalled} from '../screenshot/capture.js';

export const TOOLS = ['scribble', 'area', 'note', 'text', 'right-click', 'back'];
// The board's drawings, on a square of 24
const DRAWINGS = {
    'scribble': ['M4 20c4-1 6-6 9-9s5-4 7-3-2 4-6 6'],
    'area': ['M4 8V4h4M16 4h4v4M20 16v4h-4M8 20H4v-4'],
    'note': ['M6 3h9l4 4v14H6zM9 12h7M9 16h5'],
    'text': ['M5 6h14M12 6v13M9 19h6'],
    'right-click': ['M7 4h10a3 3 0 0 1 3 3v6a8 8 0 0 1-16 0V7a3 3 0 0 1 3-3zM12 4v6'],
    'back': ['M10 6l-6 6 6 6M4 12h16'],
};
// The ring: 196 px across, each tool 58 px across with its centre 70 px out, the first at the top
const DISC = 196;
const ITEM = 58;
const ORBIT = 70;
// Nearer the centre than this, nothing is lit; beyond the disc's edge with a tool lit, it runs
const DEAD = 24;
// The strip: 14 px in from the right edge, its top 110 px down
const STRIP_INSET = 14;
const STRIP_TOP = 110;
const OPEN_TIME = 150;
const OPEN_SCALE = 0.85;
const NOTES = 'amethystora-notes';
// Clutter's buttons: 3 the right, 8 the back button (Mutter's BTN_SIDE)
const RIGHT = 3;
const BACK = 8;

function toolName(id) {
    switch (id) {
    case 'scribble':
        return _('Scribble');
    case 'area':
        return _('Area');
    case 'note':
        return _('Note');
    case 'text':
        return _('Text');
    case 'right-click':
        return _('Right click');
    default:
        return _('Back');
    }
}

// Whether Notes has its quick note (as the hot corner checks)
function notesCapture() {
    const path = GLib.find_program_in_path(NOTES);
    if (!path)
        return false;
    try {
        const [, bytes] = GLib.file_get_contents(path);
        return new TextDecoder().decode(bytes).includes('capture');
    } catch {
        return false;
    }
}

export class PenRing {
    constructor(touch) {
        this._touch = touch;
        this._root = null;
        this._virtual = null;
    }

    get isOpen() {
        return !!this._root;
    }

    // What each tool does, or null where it cannot run here
    _runner(id) {
        switch (id) {
        case 'scribble': {
            const module = [...this._touch.modules?.values() ?? []].find(m => typeof m?.toggleScribble === 'function');
            return module ? () => module.toggleScribble() : null;
        }
        case 'area':
            return () => this._openScreenshot();
        case 'note':
            return notesCapture() ? () => Gio.Subprocess.new([NOTES, 'capture'], Gio.SubprocessFlags.NONE) : null;
        case 'text': {
            const screenshot = this._touch.modules?.get('screenshot');
            return screenshot?.openForText && gradiaInstalled() ? () => screenshot.openForText() : null;
        }
        case 'right-click':
            return () => this._click(RIGHT);
        case 'back':
            return () => this._click(BACK);
        default:
            return null;
        }
    }

    open(variant, tools) {
        if (this._root)
            return;
        const [x, y] = global.get_pointer();
        const monitor = Main.layoutManager.monitors.find(m =>
            x >= m.x && x < m.x + m.width && y >= m.y && y < m.y + m.height) ?? Main.layoutManager.primaryMonitor;
        if (!monitor)
            return;
        this._variant = variant === 'strip' ? 'strip' : 'ring';
        this._origin = [x, y];
        this._monitor = monitor;
        this._lit = -1;
        const scale = St.ThemeContext.get_for_stage(global.stage).scale_factor;

        // The whole stage, so that a press anywhere reaches it while it holds the input
        this._root = new St.Widget({name: 'amethystora-touch-ring', reactive: true,
            x: 0, y: 0, width: global.stage.width, height: global.stage.height});
        Main.layoutManager.uiGroup.add_child(this._root);
        this._items = [];
        if (this._variant === 'ring')
            this._buildRing(tools, x, y, monitor, scale);
        else
            this._buildStrip(tools, monitor, scale);

        this._grab = Main.pushModal(this._root, {actionMode: Shell.ActionMode.POPUP});
        // Another grab holds the input above it (the on-screen keyboard, a dialog): no ring that nothing reaches
        if (this._grab.is_revoked()) {
            this.close();
            return;
        }
        // Over a fullscreen window too, which would otherwise be shown straight to the screen above it
        global.compositor.disable_unredirect();
        this._unredirected = true;
        this._root.connect('motion-event', (actor, event) => this._moved(event));
        this._root.connect('button-press-event', (actor, event) => this._pressed(event));
        this._root.connect('touch-event', (actor, event) => {
            if (event.type() === Clutter.EventType.TOUCH_BEGIN)
                return this._pressed(event);
            return Clutter.EVENT_STOP;
        });
        this._root.connect('key-press-event', (actor, event) => {
            if (event.get_key_symbol() === Clutter.KEY_Escape)
                this.close();
            return Clutter.EVENT_STOP;
        });
        this._touch.surfacesChanged();
    }

    _item(id, runner, styleClass) {
        const ring = this._variant === 'ring';
        // In the ring each tool is a circle, its drawing over its word; in the strip a row as wide as the strip
        const box = new St.BoxLayout({style_class: styleClass, reactive: false,
            orientation: ring ? Clutter.Orientation.VERTICAL : Clutter.Orientation.HORIZONTAL,
            x_align: ring ? Clutter.ActorAlign.CENTER : Clutter.ActorAlign.FILL, y_align: Clutter.ActorAlign.CENTER});
        box.add_child(lineIcon(DRAWINGS[id], 'amethystora-touch-tool-icon'));
        const label = new St.Label({style_class: 'amethystora-touch-tool-label', text: toolName(id),
            x_align: ring ? Clutter.ActorAlign.CENTER : Clutter.ActorAlign.START, y_align: Clutter.ActorAlign.CENTER});
        label.clutter_text.use_markup = false;
        box.add_child(label);
        this._items.push({id, runner, actor: box});
        return box;
    }

    _buildRing(tools, x, y, monitor, scale) {
        const half = DISC * scale / 2;
        // Kept on the screen: the ring moves in, the dot stays where the pen is
        const cx = Math.max(monitor.x + half, Math.min(x, monitor.x + monitor.width - half));
        const cy = Math.max(monitor.y + half, Math.min(y, monitor.y + monitor.height - half));
        this._centre = [cx, cy];
        this._ring = new St.Widget({style_class: 'amethystora-touch-ring-disc', reactive: false,
            x: cx - half, y: cy - half, width: 2 * half, height: 2 * half});
        this._ring.set_pivot_point(0.5, 0.5);
        this._root.add_child(this._ring);
        const itemHalf = ITEM * scale / 2;
        tools.slice(0, TOOLS.length).forEach((id, k) => {
            const runner = this._runner(id);
            if (!runner)
                return;
            const angle = -Math.PI / 2 + k * Math.PI / 3;
            const box = this._item(id, runner, 'amethystora-touch-ring-tool');
            box.set_position(half + Math.cos(angle) * ORBIT * scale - itemHalf,
                half + Math.sin(angle) * ORBIT * scale - itemHalf);
            box.set_size(2 * itemHalf, 2 * itemHalf);
            this._items.at(-1).angle = angle;
            this._ring.add_child(box);
        });
        const dot = new St.Widget({style_class: 'amethystora-touch-ring-dot', reactive: false});
        this._root.add_child(dot);
        const [, size] = dot.get_preferred_width(-1);
        dot.set_position(x - size / 2, y - size / 2);
        this._dot = dot;
        this._ring.opacity = 0;
        this._ring.set_scale(OPEN_SCALE, OPEN_SCALE);
        // Reduce motion: the fade alone
        if (this._touch.reduceMotion)
            this._ring.set_scale(1, 1);
        this._ring.ease({opacity: 255, scale_x: 1, scale_y: 1, duration: OPEN_TIME,
            mode: Clutter.AnimationMode.EASE_OUT_QUAD});
    }

    _buildStrip(tools, monitor, scale) {
        this._ring = new St.BoxLayout({style_class: 'amethystora-touch-strip', reactive: false,
            orientation: Clutter.Orientation.VERTICAL});
        for (const id of tools) {
            const runner = this._runner(id);
            if (runner)
                this._ring.add_child(this._item(id, runner, 'amethystora-touch-strip-tool'));
        }
        this._root.add_child(this._ring);
        const [, width] = this._ring.get_preferred_width(-1);
        this._ring.set_position(monitor.x + monitor.width - STRIP_INSET * scale - width, monitor.y + STRIP_TOP * scale);
        this._ring.opacity = 0;
        this._ring.ease({opacity: 255, duration: OPEN_TIME, mode: Clutter.AnimationMode.EASE_OUT_QUAD});
    }

    // The tool a point is on: in the ring by its direction from the centre, in the strip by its row
    _toolAt(x, y) {
        if (this._variant === 'strip') {
            return this._items.findIndex(item => {
                const [ax, ay] = item.actor.get_transformed_position();
                const [w, h] = item.actor.get_transformed_size();
                return x >= ax && x < ax + w && y >= ay && y < ay + h;
            });
        }
        const [cx, cy] = this._centre;
        const scale = St.ThemeContext.get_for_stage(global.stage).scale_factor;
        if (Math.hypot(x - cx, y - cy) < DEAD * scale)
            return -1;
        const angle = Math.atan2(y - cy, x - cx);
        let best = -1, bestGap = Math.PI / 6;
        this._items.forEach((item, i) => {
            let gap = Math.abs(angle - item.angle) % (2 * Math.PI);
            if (gap > Math.PI)
                gap = 2 * Math.PI - gap;
            if (gap <= bestGap) {
                best = i;
                bestGap = gap;
            }
        });
        return best;
    }

    _light(index) {
        if (index === this._lit)
            return;
        this._items[this._lit]?.actor.remove_style_pseudo_class('lit');
        this._lit = index;
        this._items[index]?.actor.add_style_pseudo_class('lit');
    }

    _moved(event) {
        this._notePen(event);
        const [x, y] = event.get_coords();
        const index = this._toolAt(x, y);
        this._light(index);
        // The flick: past the disc's edge with a tool lit
        if (this._variant === 'ring' && index >= 0) {
            const [cx, cy] = this._centre;
            const scale = St.ThemeContext.get_for_stage(global.stage).scale_factor;
            if (Math.hypot(x - cx, y - cy) > DISC * scale / 2)
                this._run(index);
        }
        return Clutter.EVENT_STOP;
    }

    _pressed(event) {
        this._notePen(event);
        const [x, y] = event.get_coords();
        const index = this._toolAt(x, y);
        const inside = this._variant === 'ring'
            ? Math.hypot(x - this._centre[0], y - this._centre[1]) <= DISC * St.ThemeContext.get_for_stage(global.stage).scale_factor / 2
            : index >= 0;
        if (index >= 0 && inside)
            this._run(index);
        else
            this.close();
        return Clutter.EVENT_STOP;
    }

    _run(index) {
        const item = this._items[index];
        this.close();
        if (!item)
            return;
        // After the ring has let go of the input, so that the tool's own window or click gets it
        if (this._runIdle)
            GLib.source_remove(this._runIdle);
        this._runIdle = GLib.idle_add(GLib.PRIORITY_DEFAULT, () => {
            this._runIdle = 0;
            try {
                item.runner();
            } catch (e) {
                console.error(`Amethystora UI: the pen's ${item.id} did not run: ${e.message}`);
            }
            return GLib.SOURCE_REMOVE;
        });
    }

    // The pen that touched the ring, under the settings path Mutter keeps its buttons in (its serial number, or the
    // tablet's for a pen without one), written down for `ame desktop touch pen-button on`
    _notePen(event) {
        const tool = event.get_device_tool?.();
        const device = event.get_source_device?.();
        if (!tool || !device)
            return;
        const hex = (n, width) => Number(n).toString(16).padStart(width, '0');
        const serial = Number(tool.get_serial());
        const id = serial > 1 ? hex(serial, 1)
            : `default-${hex(device.get_vendor_id(), 4)}:${hex(device.get_product_id(), 4)}`;
        if (!/^([0-9a-f]{1,16}|default-[0-9a-f]{4}:[0-9a-f]{4})$/.test(id) || this._pens?.has(id))
            return;
        const dir = GLib.build_filenamev([GLib.get_user_state_dir(), 'amethystora']);
        const file = GLib.build_filenamev([dir, 'pens']);
        try {
            let known = [];
            if (GLib.file_test(file, GLib.FileTest.EXISTS)) {
                known = new TextDecoder().decode(GLib.file_get_contents(file)[1]).split('\n').filter(l => l);
                GLib.file_set_contents(`${file}.bak`, known.join('\n') + '\n');
            }
            this._pens = new Set(known);
            if (!this._pens.has(id)) {
                this._pens.add(id);
                GLib.mkdir_with_parents(dir, 0o700);
                GLib.file_set_contents(file, [...this._pens].slice(-32).join('\n') + '\n');
            }
        } catch (e) {
            console.warn(`Amethystora UI: the pen was not written down: ${e.message}`);
        }
    }

    // Run a tool by its name, as a press on it would (for the suites)
    runForTest(id) {
        const index = this._items?.findIndex(item => item.id === id) ?? -1;
        if (index >= 0)
            this._run(index);
        return index >= 0;
    }

    close() {
        if (!this._root)
            return;
        if (this._grab) {
            Main.popModal(this._grab);
            this._grab = null;
        }
        if (this._unredirected) {
            global.compositor.enable_unredirect();
            this._unredirected = false;
        }
        this._ring?.remove_all_transitions();
        this._root.destroy();
        this._root = null;
        this._ring = null;
        this._dot = null;
        this._items = [];
        this._lit = -1;
        this._touch.surfacesChanged();
    }

    // GNOME's screenshot UI on a selection
    _openScreenshot() {
        const ui = Main.screenshotUI;
        Promise.resolve(ui.open()).then(() => {
            if (ui._selectionButton)
                ui._selectionButton.checked = true;
        }).catch(logError);
    }

    // A click of `button` where the ring opened, from a pointer of the shell's own made for this click alone: Mutter
    // counts a pointer as a pointer, and on a touch screen without a hinge switch one that stayed would end touch mode
    // (the seat's touch-mode is "no pointer" there). Touch mode sets aside what the seat says for this moment.
    _click(button) {
        const seat = global.stage.context.get_backend().get_default_seat();
        this._virtual = seat.create_virtual_device(Clutter.InputDeviceType.POINTER_DEVICE);
        const [x, y] = this._origin;
        const now = GLib.get_monotonic_time();
        this._virtual.notify_absolute_motion(now, x, y);
        this._virtual.notify_button(now, button, Clutter.ButtonState.PRESSED);
        this._virtual.notify_button(now + 1, button, Clutter.ButtonState.RELEASED);
        // Dropped once the release has gone through; what the seat says is heeded again once it has settled
        if (this._clickIdle)
            GLib.source_remove(this._clickIdle);
        this._clickIdle = GLib.timeout_add(GLib.PRIORITY_DEFAULT, 100, () => {
            this._virtual?.run_dispose();
            this._virtual = null;
            this._clickIdle = GLib.timeout_add(GLib.PRIORITY_DEFAULT, 500, () => {
                this._clickIdle = 0;
                this._touch.mode?.resync();
                return GLib.SOURCE_REMOVE;
            });
            return GLib.SOURCE_REMOVE;
        });
    }

    // While a click of its own is under way, and the seat settles after it
    get clicking() {
        return !!this._clickIdle;
    }

    destroy() {
        this.close();
        for (const id of [this._runIdle, this._clickIdle]) {
            if (id)
                GLib.source_remove(id);
        }
        this._runIdle = this._clickIdle = 0;
        if (this._virtual) {
            this._virtual.run_dispose();
            this._virtual = null;
            this._touch.mode?.resync();
        }
    }

    glassSurfaces() {
        if (!this._ring)
            return [];
        const surfaces = [{actor: this._ring, material: 'glass', radius: null}];
        if (this._variant === 'ring') {
            for (const item of this._items)
                surfaces.push({actor: item.actor, material: 'glass', radius: null});
        }
        return surfaces;
    }

    state() {
        if (!this._root)
            return {open: false, virtual: !!this._virtual};
        return {
            open: true,
            variant: this._variant,
            tools: this._items.map(item => {
                const [x, y] = item.actor.get_transformed_position();
                const [width, height] = item.actor.get_transformed_size();
                return {id: item.id, x, y, width, height};
            }),
            centre: this._centre ?? null,
            lit: this._items[this._lit]?.id ?? null,
            virtual: !!this._virtual,
        };
    }
}
