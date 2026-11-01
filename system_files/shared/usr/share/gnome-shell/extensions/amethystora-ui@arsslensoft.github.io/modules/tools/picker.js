// Pick a colour from the screen: Super+Alt+C holds the screen still (GNOME's own frozen copy of it, as its screenshot
// tool takes) and a loupe follows the pointer, the 7 × 7 pixels round it magnified, with the colour under it named. A
// click copies it as hex, Shift+click as rgb(), Ctrl+click as hsl(); Esc, a right click or the key again copies nothing.
// The copy is kept nowhere but in memory, and the frozen copy goes when the loupe closes. Never while the screen is
// shared: what the loupe magnifies would be shown to everyone watching.

import Clutter from 'gi://Clutter';
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import Shell from 'gi://Shell';
import St from 'gi://St';

import {gettext as _} from 'resource:///org/gnome/shell/extensions/extension.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';

import {Capture} from '../pill/capture.js';

Gio._promisify(Shell.Screenshot.prototype, 'screenshot_stage_to_content');
Gio._promisify(Shell.Screenshot, 'composite_to_stream');

// The board's loupe: 7 × 7 pixels of 18 px in a circle 126 px across, a ring of 3 px round it
const CELLS = 7;
const CELL = 18;
const LOUPE = CELLS * CELL;
const RING = 3;
// How far up and right of the pointer the loupe sits, so that it never covers the pixel picked
const OFFSET = 26;
const KEPT = 8;

// The last colours picked, newest first, as hex: at module scope, so that a screen lock keeps them
const picked = [];

export function pickedColours() {
    return [...picked];
}

function hex([r, g, b]) {
    return `#${[r, g, b].map(v => v.toString(16).padStart(2, '0')).join('').toUpperCase()}`;
}

export function rgbText([r, g, b]) {
    return `rgb(${r}, ${g}, ${b})`;
}

export function hslText([r, g, b]) {
    const [rf, gf, bf] = [r / 255, g / 255, b / 255];
    const max = Math.max(rf, gf, bf), min = Math.min(rf, gf, bf);
    const l = (max + min) / 2;
    let h = 0, s = 0;
    if (max !== min) {
        const d = max - min;
        s = l > 0.5 ? d / (2 - max - min) : d / (max + min);
        if (max === rf)
            h = (gf - bf) / d + (gf < bf ? 6 : 0);
        else if (max === gf)
            h = (bf - rf) / d + 2;
        else
            h = (rf - gf) / d + 4;
        h *= 60;
    }
    return `hsl(${Math.round(h) % 360}, ${Math.round(s * 100)}%, ${Math.round(l * 100)}%)`;
}

export {hex as hexText};

// Remembers a colour picked (or picked again from the search)
export function remember(text) {
    const at = picked.indexOf(text);
    if (at >= 0)
        picked.splice(at, 1);
    picked.unshift(text);
    picked.length = Math.min(picked.length, KEPT);
}

export class ColourPicker {
    constructor(onClosed) {
        this._onClosed = onClosed;
    }

    get open() {
        return !!this._root;
    }

    // Opens the loupe; resolves once it is up (or was refused)
    async start() {
        if (this._root || this._starting)
            return;
        this._starting = true;
        try {
            this._capture = new Capture();
            this._unlisten = this._capture.listen(() => {
                if (this._capture?.sharing)
                    this.close();
            });
            if (this._capture.sharing) {
                Main.osdWindowManager.showAll(Gio.ThemedIcon.new('color-select-symbolic'),
                    _('Not while the screen is shared'), null);
                this.close();
                return;
            }
            const shooter = new Shell.Screenshot();
            const [content, scale] = await shooter.screenshot_stage_to_content();
            if (!this._capture)
                return;
            // The pixels themselves, read once from the same copy, for the loupe and the colour
            const stream = Gio.MemoryOutputStream.new_resizable();
            const pixbuf = await Shell.Screenshot.composite_to_stream(content.get_texture(), 0, 0, -1, -1, scale,
                null, 0, 0, 1, stream);
            stream.close(null);
            if (!this._capture)
                return;
            this._pixels = {
                data: pixbuf.get_pixels(),
                width: pixbuf.get_width(),
                height: pixbuf.get_height(),
                stride: pixbuf.get_rowstride(),
                channels: pixbuf.get_n_channels(),
                scale,
            };
            this._build(content);
        } catch (e) {
            console.warn(`Amethystora UI: the colour picker could not hold the screen: ${e.message}`);
            this.close();
        } finally {
            this._starting = false;
        }
    }

    _build(content) {
        const root = new St.Widget({
            style_class: 'amethystora-tools-picker',
            reactive: true,
            can_focus: true,
            x: 0, y: 0,
            width: global.stage.width,
            height: global.stage.height,
        });
        this._root = root;
        const frozen = new Clutter.Actor({
            width: global.stage.width,
            height: global.stage.height,
            minification_filter: Clutter.ScalingFilter.NEAREST,
            magnification_filter: Clutter.ScalingFilter.NEAREST,
        });
        frozen.set_content(content);
        root.add_child(frozen);

        this._loupe = new St.Widget({style_class: 'amethystora-tools-loupe', width: LOUPE + RING * 2, height: LOUPE + RING * 2});
        this._cells = new St.DrawingArea({x: RING, y: RING, width: LOUPE, height: LOUPE});
        this._cells.connect('repaint', area => this._paintCells(area));
        this._loupe.add_child(this._cells);
        root.add_child(this._loupe);

        this._label = new St.BoxLayout({style_class: 'amethystora-tools-picker-label'});
        this._swatch = new St.Widget({style_class: 'amethystora-tools-picker-swatch', y_align: Clutter.ActorAlign.CENTER});
        this._value = new St.Label({style_class: 'amethystora-tools-picker-value', y_align: Clutter.ActorAlign.CENTER});
        this._label.add_child(this._swatch);
        this._label.add_child(this._value);
        root.add_child(this._label);

        this._hint = new St.BoxLayout({style_class: 'amethystora-tools-picker-hint'});
        // Click copies hex · [Shift] rgb() · [Ctrl] hsl() · [Esc]: words, then a dot, a key and its words
        const word = text => new St.Label({text, y_align: Clutter.ActorAlign.CENTER});
        const key = text => new St.Label({text, style_class: 'amethystora-tools-picker-key', y_align: Clutter.ActorAlign.CENTER});
        const dot = () => new St.Label({text: '·', style_class: 'amethystora-tools-picker-dot', y_align: Clutter.ActorAlign.CENTER});
        for (const part of [word(_('Click copies hex')), dot(), key(_('Shift')), word('rgb()'), dot(), key(_('Ctrl')),
            word('hsl()'), dot(), key(_('Esc'))])
            this._hint.add_child(part);
        root.add_child(this._hint);

        Main.uiGroup.add_child(root);
        Main.uiGroup.set_child_above_sibling(root, null);
        // GNOME 51's grab no longer says what it holds (no get_seat_state); a grab that did not take the pointer
        // still closes on Esc and on the key
        this._grab = Main.pushModal(root, {actionMode: Shell.ActionMode.POPUP});
        root.set_cursor_type?.(Clutter.CursorType.CROSSHAIR);

        const motion = new Clutter.MotionController();
        motion.connect('motion', (controller, sprite, x, y) => this._moved(x, y));
        root.add_action(motion);
        const click = new Clutter.ClickGesture();
        click.set_recognize_on_press?.(false);
        click.connect('recognize', () => {
            const button = click.get_button?.() ?? 1;
            if (button !== 1) {
                this.close();
                return;
            }
            const point = click.get_coords_abs?.();
            const [x, y] = point ? [point.x, point.y] : global.get_pointer();
            this._picked(x, y);
        });
        // Every button, so that a right click closes it
        click.set_required_button?.(0);
        root.add_action(click);
        root.connect('key-press-event', (actor, event) => {
            if (event.get_key_symbol() === Clutter.KEY_Escape) {
                this.close();
                return Clutter.EVENT_STOP;
            }
            return Clutter.EVENT_PROPAGATE;
        });
        root.grab_key_focus();
        const [x, y] = global.get_pointer();
        this._moved(x, y);
    }

    // The colour of the pixel under stage point x, y, from the frozen copy: [r, g, b], or null outside it
    colourAt(x, y) {
        const p = this._pixels;
        if (!p)
            return null;
        const px = Math.floor(x * p.scale), py = Math.floor(y * p.scale);
        if (px < 0 || py < 0 || px >= p.width || py >= p.height)
            return null;
        const at = py * p.stride + px * p.channels;
        return [p.data[at], p.data[at + 1], p.data[at + 2]];
    }

    _moved(x, y) {
        if (!this._root)
            return;
        this._at = [x, y];
        const colour = this.colourAt(x, y);
        if (colour) {
            this._value.text = hex(colour);
            this._swatch.set_style(`background-color: ${hex(colour)};`);
        }
        this._cells.queue_repaint();
        // Up and right of the pointer, flipped near the edges of its screen
        const monitor = Main.layoutManager.monitors[global.display.get_current_monitor()] ?? Main.layoutManager.primaryMonitor;
        const size = LOUPE + RING * 2;
        const [, labelHeight] = this._label.get_preferred_height(-1);
        const [, labelWidth] = this._label.get_preferred_width(-1);
        const total = size + 10 + labelHeight;
        let lx = x + OFFSET;
        let ly = y - OFFSET - total;
        if (lx + size > monitor.x + monitor.width)
            lx = x - OFFSET - size;
        if (ly < monitor.y)
            ly = y + OFFSET;
        this._loupe.set_position(Math.round(lx), Math.round(ly));
        this._label.set_position(Math.round(lx + (size - labelWidth) / 2), Math.round(ly + size + 10));
        const [, hintWidth] = this._hint.get_preferred_width(-1);
        const [, hintHeight] = this._hint.get_preferred_height(-1);
        this._hint.set_position(Math.round(monitor.x + (monitor.width - hintWidth) / 2),
            Math.round(monitor.y + monitor.height - hintHeight - 14));
    }

    // The 7 × 7 pixels round the pointer, in a circle, with faint lines between them and the middle one outlined
    _paintCells(area) {
        const cr = area.get_context();
        try {
            const [width] = area.get_surface_size();
            const unit = width / LOUPE;
            cr.scale(unit, unit);
            cr.arc(LOUPE / 2, LOUPE / 2, LOUPE / 2, 0, 2 * Math.PI);
            cr.clip();
            const [x, y] = this._at ?? [0, 0];
            const scale = this._pixels?.scale ?? 1;
            // In the frozen copy's own pixels, so that each cell is one of them
            const px = Math.floor(x * scale), py = Math.floor(y * scale);
            const half = Math.floor(CELLS / 2);
            for (let row = 0; row < CELLS; row++) {
                for (let column = 0; column < CELLS; column++) {
                    const colour = this.colourAt((px + column - half) / scale, (py + row - half) / scale) ?? [0, 0, 0];
                    cr.setSourceRGB(colour[0] / 255, colour[1] / 255, colour[2] / 255);
                    cr.rectangle(column * CELL, row * CELL, CELL, CELL);
                    cr.fill();
                }
            }
            cr.setSourceRGBA(0, 0, 0, 0.25);
            cr.setLineWidth(0.5);
            for (let i = 1; i < CELLS; i++) {
                cr.moveTo(i * CELL, 0);
                cr.lineTo(i * CELL, LOUPE);
                cr.moveTo(0, i * CELL);
                cr.lineTo(LOUPE, i * CELL);
            }
            cr.stroke();
            cr.setSourceRGB(1, 1, 1);
            cr.setLineWidth(2);
            cr.rectangle(half * CELL + 1, half * CELL + 1, CELL - 2, CELL - 2);
            cr.stroke();
        } finally {
            cr.$dispose();
        }
    }

    _picked(x, y) {
        const colour = this.colourAt(x, y);
        if (!colour) {
            this.close();
            return;
        }
        const [,, mods] = global.get_pointer();
        let text = hex(colour);
        if (mods & Clutter.ModifierType.SHIFT_MASK)
            text = rgbText(colour);
        else if (mods & Clutter.ModifierType.CONTROL_MASK)
            text = hslText(colour);
        St.Clipboard.get_default().set_text(St.ClipboardType.CLIPBOARD, text);
        remember(text);
        this.close();
        Main.osdWindowManager.showAll(Gio.ThemedIcon.new('color-select-symbolic'), _('Copied %s').format(text), null);
    }

    close() {
        if (this._grab) {
            Main.popModal(this._grab);
            this._grab = null;
        }
        this._root?.destroy();
        this._root = null;
        this._loupe = this._cells = this._label = this._swatch = this._value = this._hint = null;
        this._pixels = null;
        this._unlisten?.();
        this._unlisten = null;
        this._capture?.destroy();
        const wasOpen = !!this._capture;
        this._capture = null;
        if (wasOpen)
            GLib.idle_add(GLib.PRIORITY_DEFAULT, () => {
                this._onClosed?.();
                return GLib.SOURCE_REMOVE;
            });
    }

    // For the suites
    get state() {
        return {open: !!this._root, colour: this._value?.text ?? null, at: this._at ?? null};
    }
}
