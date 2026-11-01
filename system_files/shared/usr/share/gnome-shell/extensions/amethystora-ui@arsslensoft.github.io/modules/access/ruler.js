// The reading ruler: a band that follows the pointer, for anyone who loses their line. It lies across the focused
// window while the pointer is over it, else across the screen the pointer is on, and is as tall as a line of text:
// 30 px at GNOME's normal text size, more when the text is made larger. Three plain actors, moved where the pointer
// goes and nothing redrawn but their place; while the ruler is hidden, nothing is made or listened to.
//
//   band       a highlighter's yellow band with a line along each edge
//   dim        the band's place clear, the window above and below it veiled
//   underline  a line in the accent under where the band would be

import Gio from 'gi://Gio';
import Meta from 'gi://Meta';
import St from 'gi://St';

import * as Main from 'resource:///org/gnome/shell/ui/main.js';

// The board's band, at GNOME's normal text size
const HEIGHT = 30;
// The underline: 3 px high, 8 px in from each end, its top 25/30 of the band down
const UNDERLINE = 3;
const UNDERLINE_INSET = 8;
const UNDERLINE_AT = 25 / 30;
const INTERFACE = 'org.gnome.desktop.interface';
export const STYLES = ['band', 'dim', 'underline'];

export class Ruler {
    constructor() {
        this._container = null;
        this._style = null;
        this._later = 0;
    }

    // The style to show, or null to show none
    show(style) {
        if (!STYLES.includes(style))
            style = null;
        if (style === this._style)
            return;
        this.hide();
        if (!style)
            return;
        this._style = style;
        this._interface = new Gio.Settings({schema_id: INTERFACE});
        this._interface.connectObject('changed::text-scaling-factor', () => this._queue(), this);
        this._container = new St.Widget({name: 'amethystora-access-ruler', reactive: false});
        const piece = name => {
            const actor = new St.Widget({style_class: `amethystora-access-ruler-${name}`, reactive: false});
            this._container.add_child(actor);
            return actor;
        };
        if (style === 'dim') {
            this._above = piece('veil');
            this._below = piece('veil');
        } else {
            this._line = piece(style);
        }
        // Over the windows, under the top bar, the menus and every other part of the shell
        const group = Main.layoutManager.uiGroup;
        group.add_child(this._container);
        group.set_child_above_sibling(this._container, global.window_group);
        this._tracker = global.backend.get_cursor_tracker();
        this._tracker.connectObject('position-invalidated', () => this._queue(), this);
        // Another window to lie across, or other screens, without the pointer moving
        global.display.connectObject('notify::focus-window', () => this._queue(), this);
        Main.layoutManager.connectObject('monitors-changed', () => this._queue(), this);
        // Over a fullscreen window too, which would otherwise be shown straight to the screen above it
        global.compositor.disable_unredirect();
        this._unredirected = true;
        this._place();
    }

    hide() {
        if (this._later) {
            global.compositor.get_laters().remove(this._later);
            this._later = 0;
        }
        this._tracker?.disconnectObject(this);
        global.display.disconnectObject(this);
        Main.layoutManager.disconnectObject(this);
        if (this._unredirected) {
            global.compositor.enable_unredirect();
            this._unredirected = false;
        }
        this._tracker = null;
        this._interface?.disconnectObject(this);
        // Disposed now rather than by the collector (dock.js says why)
        this._interface?.run_dispose();
        this._interface = null;
        this._container?.destroy();
        this._container = null;
        this._above = this._below = this._line = null;
        this._style = null;
    }

    get style() {
        return this._style;
    }

    // At most once a frame, however often the pointer moved
    _queue() {
        if (this._later || !this._container)
            return;
        this._later = global.compositor.get_laters().add(Meta.LaterType.BEFORE_REDRAW, () => {
            this._later = 0;
            this._place();
            return false;
        });
    }

    // The height of a line of text now
    get height() {
        const factor = this._interface?.get_double('text-scaling-factor') ?? 1;
        return Math.round(HEIGHT * (factor > 0 ? factor : 1) * St.ThemeContext.get_for_stage(global.stage).scale_factor);
    }

    // What the ruler lies across: the focused window while the pointer is over it, else the pointer's screen
    _area(x, y) {
        const window = global.display.focus_window;
        if (window && !window.minimized && window.showing_on_its_workspace?.()) {
            const frame = window.get_frame_rect();
            if (x >= frame.x && x < frame.x + frame.width && y >= frame.y && y < frame.y + frame.height)
                return frame;
        }
        return Main.layoutManager.monitors.find(m =>
            x >= m.x && x < m.x + m.width && y >= m.y && y < m.y + m.height) ?? Main.layoutManager.primaryMonitor;
    }

    _place() {
        if (!this._container)
            return;
        const [x, y] = global.get_pointer();
        const area = this._area(x, y);
        if (!area)
            return;
        const height = this.height;
        const top = Math.round(Math.max(area.y, Math.min(y - height / 2, area.y + area.height - height)));
        if (this._style === 'dim') {
            this._above.set_position(area.x, area.y);
            this._above.set_size(area.width, Math.max(0, top - area.y));
            this._below.set_position(area.x, top + height);
            this._below.set_size(area.width, Math.max(0, area.y + area.height - top - height));
        } else if (this._style === 'underline') {
            const inset = Math.min(UNDERLINE_INSET, area.width / 4);
            this._line.set_position(area.x + inset, top + Math.round(height * UNDERLINE_AT));
            this._line.set_size(Math.max(0, area.width - 2 * inset), UNDERLINE);
        } else {
            this._line.set_position(area.x, top);
            this._line.set_size(area.width, height);
        }
    }

    // For the suites: where each piece is
    state() {
        if (!this._container)
            return null;
        return {
            style: this._style,
            pieces: this._container.get_children().map(a => {
                const [x, y] = a.get_position();
                const [width, height] = a.get_size();
                return {x, y, width, height};
            }),
        };
    }
}
