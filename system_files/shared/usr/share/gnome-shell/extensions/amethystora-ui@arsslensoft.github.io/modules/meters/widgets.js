// The pieces the meters are drawn with: the bars in the top bar, the history charts, and the rows of the menus. None
// of them moves by itself: each is set to its new value at once and draws again only when that value changed.

import Clutter from 'gi://Clutter';
import GObject from 'gi://GObject';
import Pango from 'gi://Pango';
import St from 'gi://St';

import * as BarLevel from 'resource:///org/gnome/shell/ui/barLevel.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';

import * as Format from './format.js';

// Samples a chart holds, one per reading
export const SAMPLES = 50;
// A process name this long is cut in its column, and shown whole above it under the pointer
const LONG_NAME = 35;
// The most of a name the tip shows, in characters
const TIP_LENGTH = 400;

function scaleFactor() {
    return St.ThemeContext.get_for_stage(global.stage).scale_factor;
}

// A row of bars standing in a trough, one per value from 0 to 1. Fewer bars are wider.
export const Bars = GObject.registerClass(
class Bars extends St.BoxLayout {
    _init(count) {
        super._init({style_class: 'amethystora-meters-trough', y_align: Clutter.ActorAlign.CENTER});
        let width = 'narrow';
        if (count <= 4)
            width = 'wide';
        else if (count <= 8)
            width = 'medium';
        this.add_style_class_name(`amethystora-meters-${width}`);
        this._bars = [];
        this._heights = [];
        for (let i = 0; i < count; i++) {
            const bar = new St.Widget({style_class: 'amethystora-meters-bar', y_align: Clutter.ActorAlign.END});
            this._bars.push(bar);
            this.add_child(bar);
        }
        this._values = null;
        this._depth = 0;
        this.connect('style-changed', () => {
            this._depth = Math.max(this.get_theme_node().get_height(), 0);
            this._draw();
        });
    }

    get count() {
        return this._bars.length;
    }

    setValues(values) {
        this._values = values;
        this._draw();
    }

    // Before the first reading each bar is a hairline, so that the meter is there to be seen
    _draw() {
        this._bars.forEach((bar, i) => {
            const value = this._values?.[i];
            const height = value === undefined ? scaleFactor() : Math.ceil(value * this._depth);
            if (this._heights[i] !== height) {
                this._heights[i] = height;
                bar.set_height(height);
            }
        });
    }
});

// The history of one value as columns, the newest at the end of the reading direction. The columns grow from the
// bottom, or hang from the top for the lower half of the network's chart.
export const Chart = GObject.registerClass(
class Chart extends St.DrawingArea {
    _init(hanging = false) {
        super._init({style_class: 'amethystora-meters-chart', x_expand: true, y_expand: true});
        this._hanging = hanging;
        this._values = [];
        this._top = 1;
    }

    plot(values, top = 1) {
        this._values = values;
        this._top = top;
        this.queue_repaint();
    }

    vfunc_repaint() {
        const cr = this.get_context();
        const [width, height] = this.get_surface_size();
        if (this._top > 0) {
            const color = this.get_theme_node().get_foreground_color();
            const scale = scaleFactor();
            const gap = scale;
            const column = (width - gap * (SAMPLES - 1)) / SAMPLES;
            const rtl = this.get_text_direction() === Clutter.TextDirection.RTL;
            const first = SAMPLES - this._values.length;
            cr.setSourceRGBA(color.red / 255, color.green / 255, color.blue / 255, color.alpha / 255);
            this._values.forEach((value, i) => {
                const tall = Math.max(2 * scale, Math.min(value / this._top, 1) * height);
                const x = (first + i) * (column + gap);
                cr.rectangle(rtl ? width - x - column : x, this._hanging ? 0 : height - tall, column, tall);
            });
            cr.fill();
        }
        cr.$dispose();
    }
});

// The scale of the network's chart: the busiest moment rounded up to a power of ten, then halved while that still
// leaves room for it, down to 20 kB/s
export function networkScale(peak) {
    if (!(peak > 0))
        return 0;
    let top = 10 ** Math.ceil(Math.log10(peak));
    while (top / 2 > peak && top > 20000)
        top /= 2;
    return top;
}

function label(text, styleClass, params = {}) {
    return new St.Label({text, style_class: styleClass, ...params});
}

export function heading(text) {
    return label(text, 'amethystora-meters-heading', {x_align: Clutter.ActorAlign.CENTER});
}

// A name on the start side and its value on the end side; a right-to-left language mirrors them
export class Row {
    constructor(name, details = false) {
        this.actor = new St.BoxLayout({
            style_class: details ? 'amethystora-meters-row amethystora-meters-details' : 'amethystora-meters-row',
        });
        this.name = label(name, 'amethystora-meters-name', {x_expand: true});
        this.value = label('–', 'amethystora-meters-value');
        this.actor.add_child(this.name);
        this.actor.add_child(this.value);
    }

    set(text) {
        if (this.value.text !== text)
            this.value.text = text;
    }
}

export function capacity() {
    return new BarLevel.BarLevel({style_class: 'barlevel amethystora-meters-capacity', x_expand: true});
}

// A chart with its scale on the end side and how far back it reaches below it
export class History {
    constructor(minutes, mirrored = false) {
        this.actor = new St.BoxLayout({style_class: 'amethystora-meters-history', orientation: Clutter.Orientation.VERTICAL});
        const body = new St.BoxLayout();
        this.charts = mirrored ? [new Chart(), new Chart(true)] : [new Chart()];
        const area = new St.BoxLayout({
            style_class: mirrored ? 'amethystora-meters-plot amethystora-meters-mirrored' : 'amethystora-meters-plot',
            orientation: Clutter.Orientation.VERTICAL,
            x_expand: true,
        });
        for (const chart of this.charts)
            area.add_child(chart);
        body.add_child(area);

        const axis = new St.BoxLayout({style_class: 'amethystora-meters-axis', orientation: Clutter.Orientation.VERTICAL});
        this.marks = [Clutter.ActorAlign.START, Clutter.ActorAlign.CENTER, Clutter.ActorAlign.END].map(align => {
            const mark = label('', 'amethystora-meters-mark', {y_expand: true, y_align: align});
            axis.add_child(mark);
            return mark;
        });
        body.add_child(axis);
        this.actor.add_child(body);

        const span = new St.BoxLayout({style_class: 'amethystora-meters-span'});
        span.add_child(label(Format.minutesAgo(minutes), 'amethystora-meters-mark', {x_expand: true}));
        span.add_child(label(Format.now(), 'amethystora-meters-mark'));
        this.actor.add_child(span);
    }

    setMarks(texts) {
        this.marks.forEach((mark, i) => {
            if (mark.text !== texts[i])
                mark.text = texts[i];
        });
    }
}

// Six processes by name, with what each uses. A long name is cut, and shown whole above the row under the pointer.
export class ProcessList {
    constructor() {
        this.actor = new St.BoxLayout({style_class: 'amethystora-meters-processes', orientation: Clutter.Orientation.VERTICAL});
        this._rows = [];
        for (let i = 0; i < 6; i++) {
            const row = new Row('', true);
            row.name.add_style_class_name('amethystora-meters-process');
            row.name.clutter_text.ellipsize = Pango.EllipsizeMode.END;
            // A command reads left to right in every language
            row.name.set_text_direction(Clutter.TextDirection.LTR);
            row.name.reactive = true;
            row.name.track_hover = true;
            row.name.connect('notify::hover', actor => this._hover(actor));
            row.set('');
            this._rows.push(row);
            this.actor.add_child(row.actor);
        }
        this._tip = null;
    }

    // [name, value] pairs, at most six
    set(entries) {
        this._rows.forEach((row, i) => {
            const [name, value] = entries[i] ?? ['', ''];
            if (row.name.text !== name)
                row.name.text = name;
            row.set(value);
        });
    }

    _hover(actor) {
        this.hideTip();
        if (!actor.hover || actor.text.length <= LONG_NAME)
            return;
        // A command line can be megabytes long, and Pango would lay out all of it: the tip shows the start, wrapped
        // inside the width the stylesheet allows
        this._tip = new St.Label({
            text: actor.text.slice(0, TIP_LENGTH),
            style_class: 'amethystora-meters-tip amethystora-meters-popup',
        });
        this._tip.clutter_text.set({
            line_wrap: true,
            line_wrap_mode: Pango.WrapMode.WORD_CHAR,
            ellipsize: Pango.EllipsizeMode.NONE,
        });
        Main.uiGroup.add_child(this._tip);
        const [x, y] = actor.get_transformed_position();
        const [nameWidth] = actor.get_transformed_size();
        // Its height once wrapped at the width it will have, not the height of a single line
        const [, width] = this._tip.get_preferred_width(-1);
        const [, height] = this._tip.get_preferred_height(width);
        // Centred over the name, and kept on the name's screen
        const monitor = Main.layoutManager.findMonitorForActor(actor) ?? Main.layoutManager.primaryMonitor;
        const left = Math.min(Math.max(x + (nameWidth - width) / 2, monitor.x), monitor.x + monitor.width - width);
        this._tip.set_position(Math.round(left), Math.round(y - height - 4 * scaleFactor()));
    }

    hideTip() {
        this._tip?.destroy();
        this._tip = null;
    }
}
