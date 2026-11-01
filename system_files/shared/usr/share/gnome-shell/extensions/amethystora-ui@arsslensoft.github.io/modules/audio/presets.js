// Sound presets: while the key sound-presets is on, GNOME's output menu offers Flat, Laptop, Speech and Bass for the
// output in use, with the curve of the one it has, kept for that output only (the key presets, by the output's node
// name). A preset is PipeWire's own equaliser placed before that output; `amethystora-sound preset NAME NODE` writes
// the key and starts, restarts or stops the unit that runs it. What the row then shows is the key.
//
// The presets' filters are read from /usr/share/amethystora/sound/presets.json, the file the helper builds them from,
// so that the curve drawn is the one heard (the response of each biquad worked out from the audio EQ cookbook's
// formulas, at 48 kHz). The filters are sinks of PipeWire's own, which GNOME would list as outputs: while this runs,
// GNOME's output list leaves them out. The row is built while the menu is open and taken down when it closes.

import Clutter from 'gi://Clutter';
import Gio from 'gi://Gio';
import St from 'gi://St';

import {gettext as _} from 'resource:///org/gnome/shell/extensions/extension.js';
import * as PopupMenu from 'resource:///org/gnome/shell/ui/popupMenu.js';
import * as Volume from 'resource:///org/gnome/shell/ui/status/volume.js';

import {PRESET_PREFIX, keepMenu, outputSlider, runHelper, whenSlider} from './menu.js';

const PRESETS_FILE = '/usr/share/amethystora/sound/presets.json';
const ORDER = ['flat', 'laptop', 'speech', 'bass'];
const RATE = 48000;
// The curve's frequencies, as the board marks them
const TICKS = [60, 150, 400, 1000, 2400, 6000, 15000];
const LOW = 40;
const HIGH = 18000;
// The curve's height covers this many decibels either side of 0
const RANGE_DB = 9;
const NODE = /^[A-Za-z0-9_][A-Za-z0-9_.:+-]{0,199}$/;

// The words of a preset
function presetName(id) {
    switch (id) {
    case 'flat': return _('Flat');
    case 'laptop': return _('Laptop');
    case 'speech': return _('Speech');
    case 'bass': return _('Bass');
    default: return id;
    }
}

// A biquad's coefficients from the audio EQ cookbook, for one filter of the presets file
function coefficients({type, freq, q = 0.7, gain = 0}) {
    const w0 = 2 * Math.PI * freq / RATE;
    const cos = Math.cos(w0);
    const alpha = Math.sin(w0) / (2 * q);
    const A = Math.pow(10, gain / 40);
    const s = 2 * Math.sqrt(A) * alpha;
    switch (type) {
    case 'highpass':
        return [(1 + cos) / 2, -(1 + cos), (1 + cos) / 2, 1 + alpha, -2 * cos, 1 - alpha];
    case 'lowpass':
        return [(1 - cos) / 2, 1 - cos, (1 - cos) / 2, 1 + alpha, -2 * cos, 1 - alpha];
    case 'peaking':
        return [1 + alpha * A, -2 * cos, 1 - alpha * A, 1 + alpha / A, -2 * cos, 1 - alpha / A];
    case 'lowshelf':
        return [A * ((A + 1) - (A - 1) * cos + s), 2 * A * ((A - 1) - (A + 1) * cos), A * ((A + 1) - (A - 1) * cos - s),
            (A + 1) + (A - 1) * cos + s, -2 * ((A - 1) + (A + 1) * cos), (A + 1) + (A - 1) * cos - s];
    case 'highshelf':
        return [A * ((A + 1) + (A - 1) * cos + s), -2 * A * ((A - 1) + (A + 1) * cos), A * ((A + 1) + (A - 1) * cos - s),
            (A + 1) - (A - 1) * cos + s, 2 * ((A - 1) - (A + 1) * cos), (A + 1) - (A - 1) * cos - s];
    default:
        return [1, 0, 0, 1, 0, 0];
    }
}

// The preset's response at a frequency, in decibels: its gain and every filter's
export function responseDb(preset, f) {
    if (!preset)
        return 0;
    const w = 2 * Math.PI * f / RATE;
    const [c1, s1, c2, s2] = [Math.cos(w), Math.sin(w), Math.cos(2 * w), Math.sin(2 * w)];
    let db = preset.gain ?? 0;
    for (const filter of preset.filters ?? []) {
        const [b0, b1, b2, a0, a1, a2] = coefficients(filter);
        const nr = b0 + b1 * c1 + b2 * c2, ni = -(b1 * s1 + b2 * s2);
        const dr = a0 + a1 * c1 + a2 * c2, di = -(a1 * s1 + a2 * s2);
        db += 10 * Math.log10((nr * nr + ni * ni) / (dr * dr + di * di));
    }
    return db;
}

// Kept for the shell: read once, the first time the row is shown
let filePresets = null;

export class Presets {
    constructor(settings) {
        this._settings = settings;
    }

    enable() {
        this._mixer = Volume.getMixerControl();
        this._cancellable = new Gio.Cancellable();
        this._hidden = new Set();
        this._settings.connectObject(
            'changed::sound-presets', () => this._changed(),
            'changed::presets', () => this._fill(),
            this);
        this._unwait = whenSlider(outputSlider, slider => this._attach(slider));
    }

    _attach(slider) {
        this._slider = slider;
        this._release = keepMenu(this._slider, () => this._settings.get_boolean('sound-presets'));
        this._slider.menu.connectObject('open-state-changed', (menu, open) => (open ? this._open() : this._close()), this);
        if (this._slider.menu.isOpen)
            this._open();
    }

    disable() {
        this._unwait?.();
        this._unwait = null;
        this._cancellable?.cancel();
        this._cancellable = null;
        this._settings.disconnectObject(this);
        this._slider?.menu.disconnectObject(this);
        this._close();
        for (const item of this._hidden ?? [])
            item.visible = true;
        this._hidden?.clear();
        this._release?.();
        this._release = null;
        this._slider = null;
        this._mixer = null;
    }

    get _on() {
        return this._settings.get_boolean('sound-presets');
    }

    _changed() {
        this._slider?._sync();
        if (this._slider?.menu.isOpen) {
            this._close();
            this._open();
        }
    }

    // The output in use: its node name and its name as GNOME shows it, or null
    _output() {
        const sink = this._mixer?.get_default_sink();
        const node = sink?.get_name() ?? '';
        if (!NODE.test(node) || node.startsWith(PRESET_PREFIX))
            return null;
        const device = this._mixer.lookup_device_from_stream(sink);
        return {node, name: device?.description || sink.description || node};
    }

    _presetOf(node) {
        const map = this._settings.get_value('presets').deepUnpack();
        const id = map[node];
        return ORDER.includes(id) ? id : 'flat';
    }

    // GNOME's output list less the presets' own sinks
    _hideFilters() {
        const items = this._slider?._deviceItems;
        if (!(items instanceof Map))
            return;
        for (const [id, item] of items) {
            const device = this._mixer.lookup_output_id(id);
            const stream = device ? this._mixer.get_stream_from_device(device) : null;
            if (stream?.get_name()?.startsWith(PRESET_PREFIX) && !this._hidden.has(item)) {
                item.visible = false;
                this._hidden.add(item);
                item.connectObject('destroy', () => this._hidden?.delete(item), this);
            }
        }
    }

    _open() {
        this._hideFilters();
        if (!this._on || this._section)
            return;
        this._section = new PopupMenu.PopupMenuSection();
        this._slider.menu.addMenuItem(this._section, 1);
        this._item = new PopupMenu.PopupBaseMenuItem({reactive: false, can_focus: false,
            style_class: 'amethystora-audio-presets-item'});
        this._box = new St.BoxLayout({style_class: 'amethystora-audio-presets', orientation: Clutter.Orientation.VERTICAL,
            x_expand: true});
        this._item.add_child(this._box);
        this._section.addMenuItem(this._item);
        this._fill();
        if (!filePresets)
            this._read();
    }

    _close() {
        this._section?.destroy();
        this._section = null;
        this._item = null;
        this._box = null;
        this._buttons = null;
        this._curve = null;
    }

    _read() {
        const file = Gio.File.new_for_path(PRESETS_FILE);
        file.load_contents_async(this._cancellable, (f, result) => {
            try {
                const [, bytes] = f.load_contents_finish(result);
                const parsed = JSON.parse(new TextDecoder().decode(bytes));
                filePresets = typeof parsed === 'object' && parsed ? parsed : {};
            } catch (e) {
                if (!e.matches?.(Gio.IOErrorEnum, Gio.IOErrorEnum.CANCELLED))
                    console.warn(`Amethystora UI: ${PRESETS_FILE}: ${e.message}`);
                return;
            }
            this._curve?.queue_repaint();
        });
    }

    _fill() {
        const box = this._box;
        if (!box)
            return;
        box.destroy_all_children();
        const output = this._output();
        this._buttons = new Map();
        if (!output) {
            box.add_child(new St.Label({style_class: 'amethystora-audio-presets-note',
                text: _('Presets are for an output in use.')}));
            return;
        }
        const current = this._presetOf(output.node);
        // The output's name is the sound server's, text and never markup (St.Label's default)
        const head = new St.Label({style_class: 'amethystora-quick-heading',
            text: _('Preset · %s').format(output.name).toLocaleUpperCase()});
        head.clutter_text.line_wrap = true;
        box.add_child(head);

        const row = new St.BoxLayout({style_class: 'amethystora-audio-presets-row', x_expand: true});
        for (const id of ORDER) {
            const button = new St.Button({style_class: 'amethystora-audio-preset', label: presetName(id),
                can_focus: true, x_expand: true, toggle_mode: false});
            if (id === current) {
                button.add_style_pseudo_class('checked');
                button.accessible_name = _('%s, chosen').format(presetName(id));
            }
            button.connect('clicked', () => this.pick(id));
            row.add_child(button);
            this._buttons.set(id, button);
        }
        box.add_child(row);

        this._curve = new St.DrawingArea({style_class: 'amethystora-audio-curve', x_expand: true});
        this._curve.connect('repaint', area => this._draw(area, current));
        box.add_child(this._curve);
        const ticks = new St.BoxLayout({style_class: 'amethystora-audio-ticks', x_expand: true});
        for (const f of TICKS) {
            ticks.add_child(new St.Label({style_class: 'amethystora-audio-tick', x_expand: true,
                text: f >= 1000 ? `${(f / 1000).toLocaleString()}k` : `${f}`}));
        }
        box.add_child(ticks);
        const note = new St.Label({style_class: 'amethystora-audio-presets-note',
            text: _('Kept for this output only.')});
        note.clutter_text.line_wrap = true;
        box.add_child(note);
    }

    // The curve over 40 Hz to 18 kHz on a logarithmic scale, the line at 0 dB, in the style's colours: `color` the
    // curve, -amethystora-audio-grid the line
    _draw(area, id) {
        const cr = area.get_context();
        const [w, h] = area.get_surface_size();
        const node = area.get_theme_node();
        const fg = node.get_foreground_color();
        const [hasGrid, grid] = node.lookup_color('-amethystora-audio-grid', false);
        const rgba = c => [c.red / 255, c.green / 255, c.blue / 255, c.alpha / 255];
        const preset = id === 'flat' ? null : filePresets?.[id] ?? null;
        // The shape the preset gives the sound, less the gain before it that keeps it from clipping
        const shape = f => responseDb(preset, f) - (preset?.gain ?? 0);
        const y = db => h / 2 - (Math.max(-RANGE_DB, Math.min(RANGE_DB, db)) / RANGE_DB) * (h / 2 - 2);
        if (hasGrid)
            cr.setSourceRGBA(...rgba(grid));
        else
            cr.setSourceRGBA(1, 1, 1, 0.12);
        cr.setLineWidth(1);
        cr.moveTo(0, Math.round(h / 2) + 0.5);
        cr.lineTo(w, Math.round(h / 2) + 0.5);
        cr.stroke();
        cr.setSourceRGBA(...rgba(fg));
        cr.setLineWidth(2);
        const steps = Math.max(2, Math.round(w / 3));
        for (let i = 0; i <= steps; i++) {
            const f = LOW * Math.pow(HIGH / LOW, i / steps);
            const x = w * i / steps;
            if (i === 0)
                cr.moveTo(x, y(shape(f)));
            else
                cr.lineTo(x, y(shape(f)));
        }
        cr.stroke();
        cr.$dispose();
    }

    // A preset for the output in use, through the helper; the key it writes fills the row again
    pick(id) {
        const output = this._output();
        if (!ORDER.includes(id) || !output)
            return Promise.resolve(false);
        return runHelper(['preset', id, output.node], this._cancellable);
    }

    // --- For the suites ---------------------------------------------------------------------------------------------

    state() {
        const output = this._slider ? this._output() : null;
        return {
            on: this._settings.get_boolean('sound-presets'),
            shown: !!this._box,
            output,
            current: output ? this._presetOf(output.node) : null,
            buttons: this._buttons ? [...this._buttons].map(([id, b]) => ({id, label: b.label,
                checked: b.has_style_pseudo_class('checked')})) : [],
            hidden: this._hidden?.size ?? 0,
            menuEnabled: this._slider?.menuEnabled ?? null,
            curveRead: filePresets !== null,
        };
    }

    // The response the curve draws, for a check against the file
    curveForTest(id, f) {
        return responseDb(filePresets?.[id] ?? null, f);
    }
}
