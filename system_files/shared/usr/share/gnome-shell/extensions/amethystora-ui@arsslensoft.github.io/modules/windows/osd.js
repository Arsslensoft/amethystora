// A small pill that says what a key just did (a window's opacity, an app opened again) and goes a second later. It
// exists only while it is shown; its text is never markup.

import Clutter from 'gi://Clutter';
import GLib from 'gi://GLib';
import St from 'gi://St';

import * as Main from 'resource:///org/gnome/shell/ui/main.js';

import {capRow, label} from './util.js';

const SHOWN_MS = 1000;
// From the bottom of the screen, when it is not shown at a window (the board's)
const BOTTOM = 50;

export class Osd {
    // keys: the key caps in front of the text, or none; at: [x, y] for its bottom left corner, or a monitor's index to
    // be centred near its bottom
    show(text, {keys = null, at = null} = {}) {
        // One shown after another (a wheel turned step by step) only changes its words: building it again each time
        // cost a few milliseconds a step
        const kind = (keys ?? []).join('+');
        if (!this._pill || this._kind !== kind) {
            this.hide();
            const pill = new St.BoxLayout({style_class: 'amethystora-windows-osd', reactive: false});
            if (keys?.length)
                pill.add_child(capRow(keys, 'amethystora-windows-key'));
            this._words = label(text, 'amethystora-windows-osd-label');
            this._words.y_align = Clutter.ActorAlign.CENTER;
            pill.add_child(this._words);
            Main.layoutManager.uiGroup.add_child(pill);
            // Placed again once laid out, and whenever its words change its size, without asking for a layout now
            pill.connect('notify::width', () => this._place());
            pill.connect('notify::height', () => this._place());
            this._pill = pill;
            this._kind = kind;
        }
        this._words.text = text;
        this._at = at;
        this._place();
        if (this._timer)
            GLib.source_remove(this._timer);
        this._timer = GLib.timeout_add(GLib.PRIORITY_DEFAULT, SHOWN_MS, () => {
            this._timer = 0;
            this.hide();
            return GLib.SOURCE_REMOVE;
        });
    }

    hide() {
        if (this._timer)
            GLib.source_remove(this._timer);
        this._timer = 0;
        this._pill?.destroy();
        this._pill = null;
        this._words = null;
        this._kind = null;
    }

    // At a window's corner (its bottom left at `at`), or centred near the bottom of a monitor
    _place() {
        const pill = this._pill;
        if (!pill)
            return;
        const {width, height} = pill;
        if (Array.isArray(this._at)) {
            pill.set_position(Math.round(this._at[0]), Math.round(this._at[1] - height));
            return;
        }
        const s = St.ThemeContext.get_for_stage(global.stage).scale_factor;
        const monitor = Main.layoutManager.monitors[this._at ?? Main.layoutManager.primaryIndex] ??
            Main.layoutManager.primaryMonitor;
        pill.set_position(Math.round(monitor.x + (monitor.width - width) / 2),
            Math.round(monitor.y + monitor.height - BOTTOM * s - height));
    }

    get shown() {
        return this._pill ?? null;
    }

    get text() {
        return this._pill?.get_last_child()?.text ?? null;
    }
}
