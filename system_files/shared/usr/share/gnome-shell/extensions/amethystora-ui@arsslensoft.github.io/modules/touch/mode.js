// Touch mode: while a 2-in-1 is folded back into a tablet (or always, as the person chooses), the shell is sized for a
// fingertip: a taller top bar, larger icons in the dock, roomier menus (the class amethystora-touch on the shell's
// top group, which stylesheets/touch.css sizes up), and Today from the right edge: a handle there, and a swipe in from
// it opens the calendar menu. The first time in a session a short pill says so. Apps keep their own sizes.
//
// "Follow" is Mutter's own judgement, the seat's touch-mode: a touch screen, and the hinge's tablet switch on (or no
// pointer at all where there is no switch). GNOME's on-screen keyboard already follows the same signal. Hinges that
// say the wrong thing are why quick settings has a Touch mode tile while the feature is on: a click there forces the
// other state until the hinge next moves (or the session ends), and never writes the key.

import Clutter from 'gi://Clutter';
import GLib from 'gi://GLib';
import GObject from 'gi://GObject';
import Shell from 'gi://Shell';
import St from 'gi://St';

import {gettext as _} from 'resource:///org/gnome/shell/extensions/extension.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import {QuickToggle, SystemIndicator} from 'resource:///org/gnome/shell/ui/quickSettings.js';

export const CLASS = 'amethystora-touch';
// The board's timings: changes over 300 ms, the hint for four seconds
const EASE = 300;
const HINT_SECONDS = 4;
// The hint's centre, this far down the screen (the board's 360 of 560)
const HINT_AT = 360 / 560;
// The handle: 10 px in from the right edge
const HANDLE_INSET = 10;

// Shown once a session, whichever time touch mode first comes on; kept across a lock, which stops the module
let hintShown = false;

const TouchTile = GObject.registerClass(
class AmethystoraTouchTile extends QuickToggle {
    _init(mode) {
        super._init({title: _('Touch mode'), iconName: 'input-touchscreen-symbolic', toggleMode: false});
        this._amethystoraQuickId = 'touch-mode';
        this.connect('clicked', () => mode.force());
    }
});

export class TouchMode {
    constructor(touch) {
        this._touch = touch;
        this._active = false;
        this._forced = null;
        this._seat = null;
        this._standIn = null;
    }

    // The key's value: off, follow or on
    set(value) {
        this._value = ['follow', 'on'].includes(value) ? value : 'off';
        this._forced = null;
        const follow = this._value === 'follow';
        if (follow && !this._seat) {
            this._seat = global.stage.context.get_backend().get_default_seat();
            this._seat.connectObject('notify::touch-mode', () => {
                // The pen ring's own click flickers the seat's touch-mode for a moment: not the hinge
                if (this._touch.ring?.clicking)
                    return;
                // The hinge moved: what was forced is over
                this._forced = null;
                this._sync();
            }, this);
        } else if (!follow && this._seat) {
            this._seat.disconnectObject(this);
            this._seat = null;
        }
        this._setTile(this._value !== 'off');
        this._sync();
    }

    destroy() {
        this._seat?.disconnectObject(this);
        this._seat = null;
        this._forced = null;
        this._value = 'off';
        this._apply(false);
        if (this._relayout)
            GLib.source_remove(this._relayout);
        this._relayout = 0;
        this._setTile(false);
    }

    // What the hinge says (or what a suite stands in for it)
    get _hinge() {
        if (this._standIn !== null)
            return this._standIn;
        return !!this._seat?.touch_mode;
    }

    get _wanted() {
        if (this._forced !== null)
            return this._forced;
        if (this._value === 'on')
            return true;
        return this._value === 'follow' && this._hinge;
    }

    get active() {
        return this._active;
    }

    // The tile: the other state, until the hinge moves
    force() {
        this._forced = !this._active;
        this._sync();
    }

    // After the pen ring's click: what the seat says now, what was forced kept
    resync() {
        this._sync();
    }

    setSeatForTest(value) {
        this._standIn = value === null ? null : !!value;
        this._forced = null;
        this._sync();
    }

    _sync() {
        this._apply(this._value !== 'off' && this._wanted);
        if (this._tile) {
            this._tile.checked = this._active;
            this._tile.subtitle = this._subtitle();
        }
    }

    _subtitle() {
        if (this._forced !== null) {
            if (this._value === 'on')
                return _('Off for now');
            return this._forced ? _('On until the hinge moves') : _('Off until the hinge moves');
        }
        return this._value === 'on' ? _('Always on') : _('Follows the hinge');
    }

    _apply(on) {
        if (on === this._active)
            return;
        this._active = on;
        const group = Main.layoutManager.uiGroup;
        if (on)
            group.add_style_class_name(CLASS);
        else
            group.remove_style_class_name(CLASS);
        this._touch.dock?.setTouch?.(on);
        if (on) {
            this._makeEdge();
            if (!hintShown) {
                hintShown = true;
                this._showHint();
            }
        } else {
            this._dropEdge();
            this._dropHint();
        }
        this._relayoutBar();
        this._touch.surfacesChanged();
    }

    // The top bar's buttons can keep the size they had before the bar's font and paddings changed, which cut the
    // workspaces' names short, until something lays them out again: once the new style has settled, that is done
    _relayoutBar() {
        if (this._relayout)
            GLib.source_remove(this._relayout);
        this._relayout = GLib.timeout_add(GLib.PRIORITY_DEFAULT, 300, () => {
            this._relayout = 0;
            for (const button of Object.values(Main.panel.statusArea))
                button.queue_relayout?.();
            return GLib.SOURCE_REMOVE;
        });
    }

    // ---- Today from the right edge ----

    _makeEdge() {
        this._handle = new St.Widget({style_class: 'amethystora-touch-handle', reactive: true, opacity: 0});
        const click = new Clutter.ClickGesture();
        click.connect('recognize', () => this._openToday());
        this._handle.add_action(click);
        // Hidden over a fullscreen window, as the top bar is
        Main.layoutManager.addTopChrome(this._handle, {trackFullscreen: true});
        this._placeHandle();
        Main.layoutManager.connectObject('monitors-changed', () => this._placeHandle(), this);
        this._handle.ease({opacity: 255, duration: this._touch.reduceMotion ? 0 : EASE,
            mode: Clutter.AnimationMode.EASE_OUT_QUAD});

        this._gesture = new Shell.EdgeDragGesture({name: 'Amethystora Today right drag', side: St.Side.RIGHT});
        // From the right edge of the screen the handle is on, the one Today opens on
        this._gesture.connect('may-recognize', gesture => {
            const monitor = Main.layoutManager.primaryMonitor;
            // Where the touch is now, from its last event, in the stage's coordinates: GNOME 50's Clutter.Gesture has no
            // _abs getters, and its point getters gave far-off numbers here (the lab, a touch from the right edge)
            const [x, y] = gesture.get_point_event(0)?.get_coords() ?? [];
            return (Shell.ActionMode.NORMAL & Main.actionMode) !== 0 && !!monitor &&
                x >= monitor.x && x < monitor.x + monitor.width && y >= monitor.y && y < monitor.y + monitor.height;
        });
        this._gesture.connect('end', () => this._openToday());
        global.stage.add_action(this._gesture);
    }

    // At the right edge of the primary screen, centred on its height (again when the screens change: a tablet turned)
    _placeHandle() {
        const monitor = Main.layoutManager.primaryMonitor;
        if (!monitor || !this._handle)
            return;
        const [, width] = this._handle.get_preferred_width(-1);
        const [, height] = this._handle.get_preferred_height(-1);
        this._handle.set_position(monitor.x + monitor.width - HANDLE_INSET * this._scale - width,
            monitor.y + Math.round((monitor.height - height) / 2));
    }

    _dropEdge() {
        Main.layoutManager.disconnectObject(this);
        if (this._gesture) {
            global.stage.remove_action(this._gesture);
            this._gesture = null;
        }
        if (this._handle) {
            Main.layoutManager.removeChrome(this._handle);
            this._handle.destroy();
            this._handle = null;
        }
    }

    get _scale() {
        return St.ThemeContext.get_for_stage(global.stage).scale_factor;
    }

    _openToday() {
        const menu = Main.panel.statusArea.dateMenu?.menu;
        if (menu && !menu.isOpen)
            menu.open();
    }

    // ---- The hint, once a session ----

    _showHint() {
        this._dropHint();
        const monitor = Main.layoutManager.primaryMonitor;
        if (!monitor)
            return;
        this._hint = new St.Label({style_class: 'amethystora-touch-hint', text: _('Swipe from the right edge for Today'),
            reactive: false, opacity: 0});
        this._hint.clutter_text.use_markup = false;
        Main.layoutManager.addTopChrome(this._hint);
        const [, width] = this._hint.get_preferred_width(-1);
        const [, height] = this._hint.get_preferred_height(width);
        this._hint.set_position(monitor.x + Math.round((monitor.width - width) / 2),
            monitor.y + Math.round(monitor.height * HINT_AT - height / 2));
        this._hint.ease({opacity: 255, duration: this._touch.reduceMotion ? 0 : EASE, mode: Clutter.AnimationMode.EASE_OUT_QUAD});
        this._hintTimer = GLib.timeout_add_seconds(GLib.PRIORITY_DEFAULT, HINT_SECONDS, () => {
            this._hintTimer = 0;
            const hint = this._hint;
            hint?.ease({opacity: 0, duration: this._touch.reduceMotion ? 0 : EASE, mode: Clutter.AnimationMode.EASE_IN_QUAD,
                onStopped: () => {
                    if (this._hint === hint)
                        this._dropHint();
                }});
            return GLib.SOURCE_REMOVE;
        });
    }

    _dropHint() {
        if (this._hintTimer) {
            GLib.source_remove(this._hintTimer);
            this._hintTimer = 0;
        }
        if (this._hint) {
            const hint = this._hint;
            this._hint = null;
            hint.remove_all_transitions();
            Main.layoutManager.removeChrome(hint);
            hint.destroy();
            this._touch.surfacesChanged();
        }
    }

    // ---- The tile ----

    _setTile(shown) {
        if (shown && !this._indicator) {
            this._indicator = new SystemIndicator();
            this._tile = new TouchTile(this);
            this._indicator.quickSettingsItems.push(this._tile);
            Main.panel.statusArea.quickSettings.addExternalIndicator(this._indicator);
        } else if (!shown && this._indicator) {
            this._tile.destroy();
            this._tile = null;
            this._indicator.destroy();
            this._indicator = null;
        }
    }

    glassSurfaces() {
        return this._hint ? [{actor: this._hint, material: 'glass', radius: null}] : [];
    }

    state() {
        return {
            value: this._value,
            active: this._active,
            forced: this._forced,
            following: !!this._seat,
            tile: !!this._tile,
            handle: !!this._handle,
            gesture: !!this._gesture,
            hint: !!this._hint,
        };
    }
}
