// Pictures pinned on top (board 48, Pin a capture): a capture that stays in view above every window, on every workspace,
// while you work in the window below: copying numbers from a chart, following a recipe, matching a design. Drag one to
// move it, scroll to make it larger or smaller, Ctrl+scroll to see through it; the arrow lets clicks pass through it to
// the window below, for tracing, and the cross (or Esc, once it has been clicked) lets it go. A right click has the
// rest: copy, save, how see-through, and the same two. Up to six at once.
//
// A pinned picture is memory only: nothing is written unless it is saved, and it is gone at a log-out. Across a screen
// lock, when GNOME stops the extension, the pictures wait at module scope and come back where they were. While the whole
// screen is shared or recorded, each picture wears a small mark, since the people you share with see it too.

import Clutter from 'gi://Clutter';
import Meta from 'gi://Meta';
import Shell from 'gi://Shell';
import St from 'gi://St';

import {gettext as _} from 'resource:///org/gnome/shell/extensions/extension.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import * as PopupMenu from 'resource:///org/gnome/shell/ui/popupMenu.js';

import {Capture} from '../pill/capture.js';
import {contentOf, copyToClipboard} from './capture.js';
import {RoundedPicture} from './picture.js';

export const MOST = 6;
const RADIUS = 12;
// From the picture's top right corner to its buttons
const INSET = 6;
const MIN_SIZE = 64;
// A wheel step: a tenth larger or smaller; with Ctrl, a tenth more or less see-through, never under a quarter
const STEP = 0.1;
const MIN_OPACITY = 0.25;
const OPACITIES = {solid: 1, through: 0.7, faint: 0.4};
// A press that moves no farther than this is a click
const DRAG = 6;
// From the card: at most this share of the work area each way
const FIRST_SHARE = 0.4;

// Across a screen lock the extension stops and starts again: the pictures wait here, in the order they were pinned
let waiting = null;

const sameNumber = (a, b) => Math.abs(a - b) < 0.001;

// Pinning turned off while locked: what waited is let go, never shown again later
export function forgetWaiting() {
    waiting = null;
}

// While the screen is shared or recorded, a pinned picture (and the thumbnail) is blurred for everyone: it is a picture
// of other windows, which the share guard cannot see by app. Nothing uncovers it but the end of the share, since a
// remote viewer can send the pointer and the keys.
const SHARE_BLUR = 'amethystora-share-blur';
export function blurForShare(actor, shared) {
    if (shared && !actor.get_effect(SHARE_BLUR))
        actor.add_effect_with_name(SHARE_BLUR, new Shell.BlurEffect({mode: Shell.BlurMode.ACTOR, radius: 30, brightness: 1}));
    else if (!shared)
        actor.remove_effect_by_name(SHARE_BLUR);
}

// Over every window, under the lock screen whether or not the extension stops at a lock; not over a full screen
export function addUnderLock(actor) {
    Main.layoutManager.uiGroup.insert_child_below(actor, Main.layoutManager.screenShieldGroup);
    Main.layoutManager.trackChrome(actor, {trackFullscreen: true});
}

class Pin {
    constructor(pins, picture, {x, y, width, height, opacity = 1, through = false}) {
        this._pins = pins;
        // Only the picture's file data is kept (Copy and Save as use it), and the texture drawn from it
        this.picture = {bytes: picture.bytes, width: picture.width, height: picture.height, scale: picture.scale};
        this._content = contentOf(picture);
        this._opacity = opacity;
        this._through = through;

        this.actor = new St.Widget({
            style_class: 'amethystora-screenshot-pin',
            reactive: true,
            track_hover: true,
            can_focus: true,
            accessible_name: _('Pinned picture'),
            x, y, width, height,
        });
        this._image = new RoundedPicture(this._content, picture.width, picture.height, RADIUS, false, {
            reactive: !through,
        });
        this._image.add_constraint(new Clutter.BindConstraint({source: this.actor, coordinate: Clutter.BindCoordinate.SIZE}));
        this.actor.add_child(this._image);
        // The ring sits 2 px outside the picture, over its shadow
        this._ring = new St.Widget({style_class: 'amethystora-screenshot-pin-ring'});
        this._ring.add_constraint(new Clutter.BindConstraint({source: this.actor, coordinate: Clutter.BindCoordinate.SIZE, offset: 4}));
        this._ring.set_position(-2, -2);
        this.actor.add_child(this._ring);

        // The two buttons, in a small glass pill at the top right (top left in a right-to-left language)
        this.pill = new St.BoxLayout({style_class: 'amethystora-screenshot-pin-pill', reactive: true});
        this._shared = new St.Icon({
            style_class: 'amethystora-screenshot-pin-shared',
            icon_name: 'screen-shared-symbolic',
            icon_size: 12,
            visible: false,
            accessible_name: _('Shown in the share'),
        });
        this.pill.add_child(this._shared);
        this._throughButton = this._roundButton(pins.icon('through'), null, _('Clicks pass through'), 13,
            () => this.setThrough(!this._through));
        this._throughButton.toggle_mode = true;
        this.pill.add_child(this._throughButton);
        this.pill.add_child(this._roundButton(null, 'window-close-symbolic', _('Close'), 12, () => this.close()));
        this.actor.add_child(this.pill);
        this.actor.connectObject('notify::width', () => this._placePill(), this);
        this._placePill();

        this.actor.connectObject(
            'button-press-event', (a, event) => this._press(event),
            'scroll-event', (a, event) => this._scroll(event),
            'key-press-event', (a, event) => this._key(event),
            'notify::hover', () => this._hoverChanged(),
            'destroy', () => this._destroyed(), this);

        addUnderLock(this.actor);
        this.actor.opacity = Math.round(this._opacity * 255);
        this._sync();
    }

    get state() {
        return {
            x: this.actor.x, y: this.actor.y, width: this.actor.width, height: this.actor.height,
            opacity: this._opacity, through: this._through, shared: this._shared.visible,
        };
    }

    _roundButton(gicon, iconName, name, size, action) {
        const icon = gicon ? new St.Icon({gicon, icon_size: size}) : new St.Icon({icon_name: iconName, icon_size: size});
        const button = new St.Button({
            style_class: 'amethystora-screenshot-pin-button',
            child: icon,
            accessible_name: name,
            can_focus: true,
        });
        button.connect('clicked', action);
        return button;
    }

    _placePill() {
        const [, width] = this.pill.get_preferred_width(-1);
        const rtl = Clutter.get_default_text_direction() === Clutter.TextDirection.RTL;
        this.pill.set_position(rtl ? INSET : this.actor.width - INSET - width, INSET);
    }

    _sync() {
        this._image.reactive = !this._through;
        this._throughButton.checked = this._through;
        if (this._through)
            this.actor.add_style_class_name('amethystora-screenshot-pin-through');
        else
            this.actor.remove_style_class_name('amethystora-screenshot-pin-through');
        // Passing clicks through, the frame itself takes none either: only the pill does
        this.actor.reactive = !this._through;
    }

    setThrough(through) {
        this._through = through;
        this._sync();
    }

    setOpacity(opacity) {
        this._opacity = Math.clamp(opacity, MIN_OPACITY, 1);
        this.actor.opacity = Math.round(this._opacity * 255);
    }

    setShared(shared) {
        this._shared.visible = shared;
        blurForShare(this._image, shared);
        this._placePill();
    }

    // ---- The pointer ----

    // A press: the primary button drags, a click gives the keyboard to the picture (for Esc); the secondary opens the
    // menu. The pointer is the picture's only while the button is down.
    _press(event) {
        const button = event.get_button();
        if (button === Clutter.BUTTON_SECONDARY) {
            this._openMenu(event);
            return Clutter.EVENT_STOP;
        }
        if (button !== Clutter.BUTTON_PRIMARY || this._grab)
            return Clutter.EVENT_PROPAGATE;
        // A press on the pill is its buttons': their click gestures see it too, and a grab here would take the click
        // from them
        if (this.pill.contains(global.stage.get_event_actor(event)))
            return Clutter.EVENT_PROPAGATE;
        const [x, y] = event.get_coords();
        this._start = {x, y, ax: this.actor.x, ay: this.actor.y};
        this._dragging = false;
        this._grab = global.stage.grab(this.actor);
        this._pressOwner = {};
        this.actor.connectObject(
            'motion-event', (a, e) => this._motion(e),
            'button-release-event', () => this._endPress(), this._pressOwner);
        Main.layoutManager.connectObject('system-modal-opened', () => this._endPress(), this._pressOwner);
        return Clutter.EVENT_STOP;
    }

    _motion(event) {
        const [x, y] = event.get_coords();
        const start = this._start;
        if (!this._dragging && Math.hypot(x - start.x, y - start.y) > DRAG)
            this._dragging = true;
        // Moved by translation while dragged, placed once let go
        if (this._dragging)
            this.actor.set_translation(Math.round(x - start.x), Math.round(y - start.y), 0);
        return Clutter.EVENT_STOP;
    }

    _endPress() {
        if (this._pressOwner) {
            this.actor.disconnectObject(this._pressOwner);
            Main.layoutManager.disconnectObject(this._pressOwner);
        }
        this._pressOwner = null;
        this._grab?.dismiss();
        this._grab = null;
        if (this._dragging) {
            const {translation_x: tx, translation_y: ty} = this.actor;
            this.actor.set_translation(0, 0, 0);
            this.actor.set_position(this._start.ax + tx, this._start.ay + ty);
        }
        // A click: the keyboard is the picture's while the pointer stays on it, so that Esc lets it go
        if (!this._dragging && this.actor.hover)
            this.actor.grab_key_focus();
        this._dragging = false;
        return Clutter.EVENT_STOP;
    }

    // The keyboard goes back to the window as soon as the pointer leaves, so that typing never lands here by surprise
    _hoverChanged() {
        if (!this.actor.hover && global.stage.key_focus === this.actor)
            global.stage.set_key_focus(null);
    }

    _key(event) {
        if (event.get_key_symbol() === Clutter.KEY_Escape) {
            this.close();
            return Clutter.EVENT_STOP;
        }
        return Clutter.EVENT_PROPAGATE;
    }

    // Larger or smaller round the point under the pointer; with Ctrl, more or less see-through
    _scroll(event) {
        // A wheel sends a step and its smooth twin: the copy Clutter made of the other is left out
        if (event.get_flags() & Clutter.EventFlags.FLAG_POINTER_EMULATED)
            return Clutter.EVENT_STOP;
        const direction = event.get_scroll_direction();
        let sign = 0;
        if (direction === Clutter.ScrollDirection.UP)
            sign = 1;
        else if (direction === Clutter.ScrollDirection.DOWN)
            sign = -1;
        else if (direction === Clutter.ScrollDirection.SMOOTH) {
            // A touchpad sends many small steps: one step for each whole unit they add up to
            this._smooth = (this._smooth ?? 0) + event.get_scroll_delta()[1];
            if (Math.abs(this._smooth) >= 1) {
                sign = -Math.sign(this._smooth);
                this._smooth -= Math.trunc(this._smooth);
            }
        }
        if (!sign)
            return Clutter.EVENT_STOP;
        if (event.get_state() & Clutter.ModifierType.CONTROL_MASK) {
            this.setOpacity(Math.round((this._opacity + sign * STEP) * 10) / 10);
            return Clutter.EVENT_STOP;
        }
        const [px, py] = event.get_coords();
        this.resize(1 + sign * STEP, px, py);
        return Clutter.EVENT_STOP;
    }

    // By a factor, keeping the point (px, py) where it is; never smaller than 64 px nor larger than its screen
    resize(factor, px, py) {
        const {x, y, width, height} = this.actor;
        const monitor = Main.layoutManager.monitors[Main.layoutManager.findIndexForActor(this.actor)] ??
            Main.layoutManager.primaryMonitor;
        const ratio = width / Math.max(1, height);
        let w = width * factor;
        w = Math.clamp(w, Math.max(MIN_SIZE, MIN_SIZE * ratio), Math.min(monitor.width, monitor.height * ratio));
        const h = w / ratio;
        const scale = w / Math.max(1, width);
        px ??= x + width / 2;
        py ??= y + height / 2;
        this.actor.set_position(Math.round(px - (px - x) * scale), Math.round(py - (py - y) * scale));
        this.actor.set_size(Math.round(w), Math.round(h));
    }

    // ---- The menu ----

    _openMenu(event) {
        this._closeMenu();
        const [x, y] = event.get_coords();
        Main.layoutManager.setDummyCursorGeometry(x, y, 0, 0);
        const menu = new PopupMenu.PopupMenu(Main.layoutManager.dummyCursor, 0, St.Side.TOP);
        menu.actor.add_style_class_name('amethystora-screenshot-pin-menu');
        menu.addAction(_('Copy'), () => copyToClipboard(this.picture));
        menu.addAction(_('Save as…'), () => this._pins.saveAs(this.picture));
        menu.addMenuItem(new PopupMenu.PopupSeparatorMenuItem());
        const opacities = [['solid', _('Solid')], ['through', _('See-through')], ['faint', _('Faint')]];
        for (const [id, label] of opacities) {
            const item = menu.addAction(label, () => this.setOpacity(OPACITIES[id]));
            item.setOrnament(sameNumber(this._opacity, OPACITIES[id])
                ? PopupMenu.Ornament.DOT : PopupMenu.Ornament.NO_DOT);
        }
        menu.addMenuItem(new PopupMenu.PopupSeparatorMenuItem());
        const through = new PopupMenu.PopupSwitchMenuItem(_('Clicks pass through'), this._through);
        through.connect('toggled', (item, state) => this.setThrough(state));
        menu.addMenuItem(through);
        menu.addAction(_('Close'), () => this.close());
        Main.uiGroup.add_child(menu.actor);
        this._manager = new PopupMenu.PopupMenuManager(Main.layoutManager.dummyCursor);
        this._manager.addMenu(menu);
        menu.connect('open-state-changed', (m, open) => {
            if (!open && this._menu === m)
                this._closeMenuLater();
        });
        this._menu = menu;
        menu.open();
    }

    // Closed by a choice or a click elsewhere: taken away once the choice has run
    _closeMenuLater() {
        const menu = this._menu;
        if (!menu)
            return;
        this._menuLater = global.compositor.get_laters().add(Meta.LaterType.BEFORE_REDRAW, () => {
            this._menuLater = 0;
            if (this._menu === menu)
                this._closeMenu();
            return false;
        });
    }

    _closeMenu() {
        if (this._menuLater)
            global.compositor.get_laters().remove(this._menuLater);
        this._menuLater = 0;
        const menu = this._menu;
        this._menu = null;
        if (menu) {
            this._manager?.removeMenu(menu);
            menu.destroy();
        }
        this._manager = null;
    }

    // ---- Going ----

    close() {
        this.actor.destroy();
    }

    _destroyed() {
        this._closeMenu();
        if (this._pressOwner) {
            Main.layoutManager.disconnectObject(this._pressOwner);
            this._pressOwner = null;
        }
        this._grab?.dismiss();
        this._grab = null;
        if (global.stage.key_focus === this.actor)
            global.stage.set_key_focus(null);
        // The texture now, not when the collector comes by
        this._content?.run_dispose();
        this._content = null;
        this._pins._gone(this);
    }
}

export class Pins {
    // module: the screenshot module, which gives the icons and the file chooser
    constructor(module) {
        this._module = module;
        this._pins = [];
        this._capture = null;
    }

    // Puts back what waited across a lock
    enable() {
        const list = waiting ?? [];
        waiting = null;
        for (const {picture, place} of list)
            this._add(picture, place);
    }

    // Everything goes; across a lock it waits to come back
    disable() {
        waiting = Main.sessionMode.isLocked && this._pins.length > 0
            ? this._pins.map(pin => ({picture: pin.picture, place: pin.state}))
            : null;
        this._stopWatching();
        for (const pin of [...this._pins])
            pin.close();
        this._pins = [];
    }

    // Lets every picture go (pinning turned off)
    clear() {
        for (const pin of [...this._pins])
            pin.close();
    }

    get count() {
        return this._pins.length;
    }

    get list() {
        return this._pins;
    }

    icon(name) {
        return this._module.icon(name);
    }

    saveAs(picture) {
        this._module.saveAs(picture);
    }

    // From the card: at its own size, at most 40 % of the work area each way, its corner at `place`'s
    // From the bar: exactly the area, `place` being {x, y, width, height}
    pin(picture, place) {
        if (place.width === undefined) {
            const area = Main.layoutManager.getWorkAreaForMonitor(Main.layoutManager.primaryIndex);
            const scale = picture.scale || 1;
            let width = picture.width / scale;
            let height = picture.height / scale;
            const fit = Math.min(1, area.width * FIRST_SHARE / width, area.height * FIRST_SHARE / height);
            width = Math.max(MIN_SIZE, Math.round(width * fit));
            height = Math.max(MIN_SIZE, Math.round(height * fit));
            const x = place.corner === 'bottom-left' ? place.x : place.x - width;
            place = {
                x: Math.round(Math.clamp(x, area.x, area.x + area.width - width)),
                y: Math.round(Math.clamp(place.y - height, area.y, area.y + area.height - height)),
                width, height,
            };
        }
        return this._add(picture, place);
    }

    _add(picture, place) {
        // The oldest lets go for a seventh
        while (this._pins.length >= MOST)
            this._pins[0].close();
        const pin = new Pin(this, picture, place);
        this._pins.push(pin);
        this._watch();
        this._module.surfacesChanged();
        return pin;
    }

    _gone(pin) {
        const at = this._pins.indexOf(pin);
        if (at < 0)
            return;
        this._pins.splice(at, 1);
        if (this._pins.length === 0)
            this._stopWatching();
        this._module.surfacesChanged();
    }

    // The share mark: whether the screen is shared or recorded, watched only while something is pinned
    _watch() {
        if (this._capture)
            return;
        this._capture = new Capture();
        this._unlisten = this._capture.listen(() => this._syncShared());
        this._syncShared();
    }

    _stopWatching() {
        this._unlisten?.();
        this._unlisten = null;
        this._capture = null;
    }

    _syncShared() {
        const shared = this._capture?.active ?? false;
        for (const pin of this._pins)
            pin.setShared(shared);
    }

    glassSurfaces() {
        return this._pins.map(pin => ({actor: pin.pill, material: 'glass', radius: null}));
    }
}
