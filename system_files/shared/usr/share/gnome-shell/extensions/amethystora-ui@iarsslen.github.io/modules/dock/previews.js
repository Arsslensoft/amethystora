// Previews of an app's windows (spec 1.7): rest the pointer on an item with windows for 400 ms and a glass card above
// it shows them live, most recently used first, six at most and a "+N" for the rest, which opens the fan. A click on
// one brings it forward. The pictures are clones of the windows' own buffers, so nothing is copied; the card and its
// clones exist only while it is shown, and nothing runs for it while the pointer is elsewhere.

import Clutter from 'gi://Clutter';
import GLib from 'gi://GLib';
import Pango from 'gi://Pango';
import St from 'gi://St';

import * as Main from 'resource:///org/gnome/shell/ui/main.js';

import {placeCard, popIn, popOut} from './cards.js';
import {scaleFactor} from './util.js';

const REST = 400;
const GRACE = 150;
const MOST = 6;
const GAP = 14;
const SHOW = 220;
const FROM = 10;
// The picture's room in a tile, inside its border and below its title
const PICTURE = {width: 148, height: 74};

// A window's live picture fitted inside width x height (logical px), its frame without the shadow around it
export function windowPicture(window, width, height) {
    const sf = scaleFactor();
    const holder = new St.Widget({
        layout_manager: new Clutter.FixedLayout(),
        width: width * sf,
        height: height * sf,
        clip_to_allocation: true,
    });
    const actor = window.get_compositor_private();
    if (!actor)
        return holder;
    const frame = window.get_frame_rect();
    const buffer = window.get_buffer_rect();
    if (frame.width < 1 || frame.height < 1)
        return holder;
    const fit = Math.min(width * sf / frame.width, height * sf / frame.height);
    const clone = new Clutter.Clone({source: actor});
    const [w, h] = actor.get_size();
    clone.set_size(w * fit, h * fit);
    const shown = {width: frame.width * fit, height: frame.height * fit};
    clone.set_position(
        Math.round((width * sf - shown.width) / 2 - (frame.x - buffer.x) * fit),
        Math.round((height * sf - shown.height) / 2 - (frame.y - buffer.y) * fit));
    holder.add_child(clone);
    return holder;
}

// A window's title as plain text
export function titleLabel(window, styleClass, fallback) {
    const label = new St.Label({style_class: styleClass, x_expand: true, y_align: Clutter.ActorAlign.CENTER});
    label.clutter_text.use_markup = false;
    label.clutter_text.ellipsize = Pango.EllipsizeMode.END;
    const sync = () => (label.text = window.title || fallback);
    sync();
    window.connectObject('notify::title', sync, label);
    return label;
}

export class Previews {
    constructor(dash, dock) {
        this._dash = dash;
        this._dock = dock;
        this._card = null;
        this._state = null;
    }

    destroy() {
        this._clearTimers();
        this.hide(true);
    }

    get shown() {
        return Boolean(this._card);
    }

    get windows() {
        return this._windows?.length ?? 0;
    }

    get card() {
        return this._card;
    }

    // An item's button was pointed at, or no longer
    hovered(state) {
        // The light desktop (modules/comfort.js) has no live previews
        if (!this._dock.settings.get_boolean('previews') || this._dock.ext.modules?.get('comfort')?.light)
            return;
        if (state.button.hover) {
            if (this._state === state && this._card) {
                this._clearHide();
                return;
            }
            this._clearRest();
            if (!this._dash.windowsOf(state.key).length || this._dash.previewsHeld(state))
                return;
            this._restId = GLib.timeout_add(GLib.PRIORITY_DEFAULT, REST, () => {
                this._restId = 0;
                if (state.button.hover && !this._dash.previewsHeld(state))
                    this._show(state);
                return GLib.SOURCE_REMOVE;
            });
        } else {
            this._clearRest();
            if (this._state === state)
                this._queueHide();
        }
    }

    _clearRest() {
        if (this._restId)
            GLib.source_remove(this._restId);
        this._restId = 0;
    }

    _clearHide() {
        if (this._hideId)
            GLib.source_remove(this._hideId);
        this._hideId = 0;
    }

    _clearTimers() {
        this._clearRest();
        this._clearHide();
    }

    _queueHide() {
        if (this._hideId || !this._card)
            return;
        this._hideId = GLib.timeout_add(GLib.PRIORITY_DEFAULT, GRACE, () => {
            this._hideId = 0;
            if (!this._card?.hover && !this._state?.button.hover)
                this.hide();
            return GLib.SOURCE_REMOVE;
        });
    }

    _show(state) {
        this.hide(true);
        const windows = this._dash.windowsOf(state.key);
        if (!windows.length)
            return;
        this._state = state;
        this._windows = windows;
        const vertical = this._dash.edge.vertical;
        const card = new St.BoxLayout({
            style_class: 'amethystora-dock-previews',
            orientation: vertical ? Clutter.Orientation.VERTICAL : Clutter.Orientation.HORIZONTAL,
            reactive: true,
            track_hover: true,
        });
        this._card = card;
        this._fill(card, windows, state);
        card.connect('notify::hover', () => {
            if (card.hover)
                this._clearHide();
            else
                this._queueHide();
        });
        Main.layoutManager.addChrome(card);
        placeCard(card, this._dash, state.restCentre(), GAP);
        popIn(card, this._dash, {from: FROM, duration: SHOW, level: this._dock.motionLevel()});

        // Anything that takes over the dock closes it
        Main.overview.connectObject('showing', () => this.hide(), this);
        Main.xdndHandler.connectObject('drag-begin', () => this.hide(), this);
        Main.overview.connectObject('item-drag-begin', () => this.hide(), this);
        for (const window of windows)
            window.connectObject('unmanaged', () => this._refill(), this);

        this._dock.glassChanged();
    }

    _fill(card, windows, state) {
        const fallback = state.name();
        for (const window of windows.slice(0, MOST)) {
            const tile = new St.Button({style_class: 'amethystora-dock-preview', can_focus: true});
            const box = new St.BoxLayout({orientation: Clutter.Orientation.VERTICAL, x_expand: true});
            const title = new St.Bin({style_class: 'amethystora-dock-preview-title', x_expand: true});
            title.child = titleLabel(window, 'amethystora-dock-preview-name', fallback);
            box.add_child(title);
            box.add_child(windowPicture(window, PICTURE.width, PICTURE.height));
            tile.child = box;
            tile.accessible_name = window.title || fallback;
            tile.connect('clicked', () => {
                Main.activateWindow(window);
                this.hide();
            });
            card.add_child(tile);
        }
        if (windows.length > MOST) {
            const more = new St.Button({
                style_class: 'amethystora-dock-preview amethystora-dock-preview-more',
                label: `+${(windows.length - MOST).toLocaleString()}`,
                can_focus: true,
            });
            more.connect('clicked', () => {
                this.hide(true);
                this._dash.openFan(state);
            });
            card.add_child(more);
        }
    }

    // A window went: the card shows the rest, and goes with the last
    _refill() {
        const state = this._state;
        if (!this._card || !state)
            return;
        const windows = this._dash.windowsOf(state.key).filter(window => window.get_compositor_private());
        if (!windows.length) {
            this.hide();
            return;
        }
        for (const window of this._windows ?? [])
            window.disconnectObject(this);
        this._windows = windows;
        this._card.destroy_all_children();
        this._fill(this._card, windows, state);
        for (const window of windows)
            window.connectObject('unmanaged', () => this._refill(), this);
        placeCard(this._card, this._dash, state.restCentre(), GAP);
    }

    hide(now = false) {
        this._clearTimers();
        Main.overview.disconnectObject(this);
        Main.xdndHandler.disconnectObject(this);
        for (const window of this._windows ?? [])
            window.disconnectObject(this);
        this._windows = null;
        this._state = null;
        const card = this._card;
        this._card = null;
        if (!card)
            return;
        if (now) {
            card.destroy();
            this._dock.glassChanged();
            return;
        }
        popOut(card, this._dash, {
            to: FROM, duration: SHOW, level: this._dock.motionLevel(),
            onComplete: () => this._dock.glassChanged(),
        });
    }
}
