// A tip the first time a feature can help, and never again: a small glass card next to what it is about, with Got it,
// that closes by itself after twelve seconds. The features own their tips and offer one from a signal they already
// have (offer()); this module decides whether it shows. One tip per feature ever, none in the first hour after the
// module first ran for the account, never two at once, half an hour at least between two, and none while a window is
// fullscreen on that screen, the screen is shared or recorded, notifications are silenced, or the overview or a dialog
// is up. A tip never takes input from anything else: no grab, no focus, only the card itself takes a click.
//
// Two tips are this module's own: Super+Space, the third time in a session an app is started from a later page of the
// app grid, and the Scripts menu of Files, the first time Files has the focus.

import Atk from 'gi://Atk';
import Clutter from 'gi://Clutter';
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import Shell from 'gi://Shell';
import St from 'gi://St';

import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import {gettext as _} from 'resource:///org/gnome/shell/extensions/extension.js';

import {FILE, HintState} from './hints/state.js';

// The tips there are, each owned by the feature it is about (pointer, windows, connect, or this module)
export const KNOWN = ['shake', 'hold-super', 'search', 'snap', 'folder-colour', 'phone-drop', 'pinch'];

const FIRST_HOUR = 3600;
const BETWEEN = 1800;
const SHOWN_FOR = 12000;
const RECORDED_FOR = 1600;
// The card's place from the pointer's tip, and its distance from an actor and from a screen's edges
const FROM_POINTER = [40, 12];
const GAP = 12;
const INSIDE = 16;
// How many times in a session before the tip of Super+Space is offered
const SEARCH_AFTER = 3;
const FILES = 'org.gnome.Nautilus';

const now = () => Math.floor(GLib.get_real_time() / 1e6);

// Destroyed once: a card or pill may be fading when the module stops
const GONE = new WeakSet();
function gone(actor) {
    if (GONE.has(actor))
        return;
    GONE.add(actor);
    actor.destroy();
}

export class Hints {
    constructor(ext) {
        this._ext = ext;
    }

    enable() {
        this._settings = this._ext.getSettings('org.gnome.shell.extensions.amethystora-ui.hints');
        this._settings.connectObject('changed::enabled', () => this._sync(), this);
        this._sync();
    }

    disable() {
        this._settings?.disconnectObject(this);
        this._stop();
        // Disposed now rather than by the collector (dock.js says why)
        this._settings?.run_dispose();
        this._settings = null;
    }

    get enabled() {
        return !!this._state;
    }

    _sync() {
        if (this._settings.get_boolean('enabled'))
            this._start();
        else
            this._stop();
    }

    _start() {
        if (this._state)
            return;
        this._state = new HintState(KNOWN);
        this._state.begin(now());
        this._banners = new Gio.Settings({schema_id: 'org.gnome.desktop.notifications'});
        // `ame desktop hints reset` changes the file under the running shell
        this._monitor = Gio.File.new_for_path(FILE).monitor_file(Gio.FileMonitorFlags.NONE, null);
        this._monitor.connectObject('changed', () => this._state?.load(), this);
        this._searches = 0;
        this._watchOwn();
        // The owners that arm a watch of their own only while tips are on
        this._ext.modules?.get('pointer')?.armTip?.();
    }

    _stop() {
        this._ext.modules?.get('pointer')?.disarmTip?.();
        this._close(true);
        this._unwatchOwn();
        if (this._waiting)
            Main.overview.disconnectObject(this._waiting);
        this._waiting = null;
        if (this._idleId)
            GLib.source_remove(this._idleId);
        this._idleId = 0;
        this._monitor?.disconnectObject(this);
        this._monitor?.cancel();
        this._monitor = null;
        this._banners?.run_dispose();
        this._banners = null;
        this._state = null;
    }

    // Whether a tip was ever shown, for an owner that can stop listening once it has been
    wasShown(id) {
        return !!this._state?.shown[id];
    }

    // A feature's tip, shown when the rules allow it now: true when it shows. text is the owner's translated sentence;
    // anchor is 'pointer', an actor the tip is about, or null for the top of the main screen under the bar.
    offer(id, {text, anchor = null} = {}) {
        const state = this._state;
        if (!state || !KNOWN.includes(id) || typeof text !== 'string' || !text)
            return false;
        const time = now();
        if (state.shown[id] || time - state.since < FIRST_HOUR || this._card || this._pill ||
            time - state.last < BETWEEN)
            return false;
        const monitor = this._monitorOf(anchor);
        if (monitor < 0 || global.display.get_monitor_in_fullscreen(monitor))
            return false;
        if (Main.overview.visible || Main.modalCount > 0 || !this._banners.get_boolean('show-banners'))
            return false;
        let sharing = false;
        try {
            sharing = !!this._ext.modules.get('pill')?.sharing?.();
        } catch {}
        if (sharing)
            return false;

        state.record(id, time);
        this._show(text, anchor, monitor);
        return true;
    }

    // A tip that comes from something done in the overview, offered once it has closed: GNOME lets go of its grab only
    // after it says so, so the offer waits for the main loop to be idle once
    offerWhenClosed(id, options) {
        if (!this._state || !KNOWN.includes(id) || this.wasShown(id))
            return;
        const fresh = !this._waiting;
        this._waiting ??= new Map();
        this._waiting.set(id, options);
        if (!fresh)
            return;
        const later = () => {
            this._idleId ||= GLib.idle_add(GLib.PRIORITY_DEFAULT_IDLE, () => {
                this._idleId = 0;
                const waiting = this._waiting;
                this._waiting = null;
                for (const [waitingId, waitingOptions] of waiting ?? []) {
                    if (this.offer(waitingId, waitingOptions))
                        break;
                }
                return GLib.SOURCE_REMOVE;
            });
        };
        if (!Main.overview.visible) {
            later();
            return;
        }
        Main.overview.connectObject('hidden', () => {
            Main.overview.disconnectObject(this._waiting);
            later();
        }, this._waiting);
    }

    // --- The card ----------------------------------------------------------------------------------------------------

    _monitorOf(anchor) {
        if (anchor === 'pointer')
            return global.display.get_current_monitor();
        if (anchor instanceof Clutter.Actor)
            return Main.layoutManager.findIndexForActor(anchor);
        return Main.layoutManager.primaryIndex;
    }

    _show(text, anchor, monitorIndex) {
        const card = new St.BoxLayout({
            style_class: 'amethystora-hints-card',
            orientation: Clutter.Orientation.VERTICAL,
            reactive: true,
            opacity: 0,
            translation_y: 6,
        });
        card.accessible_role = Atk.Role.NOTIFICATION;
        card.accessible_name = text;
        const row = new St.BoxLayout({style_class: 'amethystora-hints-row'});
        const bulb = new St.Bin({
            style_class: 'amethystora-hints-bulb',
            y_align: Clutter.ActorAlign.START,
            child: new St.Icon({
                style_class: 'amethystora-hints-bulb-icon',
                gicon: Gio.FileIcon.new(this._ext.dir.get_child('icons').get_child('hints')
                    .get_child('amethystora-hint-symbolic.svg')),
                icon_size: 14,
            }),
        });
        const label = new St.Label({style_class: 'amethystora-hints-text', text, x_expand: true});
        label.clutter_text.use_markup = false;
        label.clutter_text.line_wrap = true;
        label.clutter_text.ellipsize = 0;
        row.add_child(bulb);
        row.add_child(label);
        const button = new St.Button({
            style_class: 'amethystora-hints-button',
            label: _('Got it'),
            x_align: Clutter.ActorAlign.END,
            can_focus: true,
            accessible_name: _('Got it, do not show this tip again'),
        });
        button.connectObject('clicked', () => this._gotIt(), this);
        card.add_child(row);
        card.add_child(button);
        Main.layoutManager.addTopChrome(card);
        this._card = card;
        this._place(card, anchor, monitorIndex);

        // Gone when the screen it is on goes fullscreen, or the overview opens over it
        global.display.connectObject('in-fullscreen-changed', () => {
            if (global.display.get_monitor_in_fullscreen(this._cardMonitor))
                this._close(false);
        }, this);
        Main.overview.connectObject('showing', () => this._close(false), this);
        this._cardMonitor = monitorIndex;

        card.ease({opacity: 255, translation_y: 0, duration: 200, mode: Clutter.AnimationMode.EASE_OUT_QUAD});
        this._timeout = GLib.timeout_add(GLib.PRIORITY_DEFAULT, SHOWN_FOR, () => {
            this._timeout = 0;
            this._close(false);
            return GLib.SOURCE_REMOVE;
        });
        this._glass();
    }

    // Next to what it is about, kept INSIDE pixels within its screen
    _place(card, anchor, monitorIndex) {
        const monitor = Main.layoutManager.monitors[monitorIndex];
        const [, width] = card.get_preferred_width(-1);
        const [, height] = card.get_preferred_height(width);
        const left = monitor.x + INSIDE, right = monitor.x + monitor.width - INSIDE;
        const top = monitor.y + INSIDE, bottom = monitor.y + monitor.height - INSIDE;
        const rtl = Clutter.get_default_text_direction() === Clutter.TextDirection.RTL;
        let x, y;
        if (anchor === 'pointer') {
            const [px, py] = global.get_pointer();
            // Beside the pointer on the reading side, flipped when it would leave the screen
            x = rtl ? px - FROM_POINTER[0] - width : px + FROM_POINTER[0];
            if (x + width > right || x < left)
                x = rtl ? px + FROM_POINTER[0] : px - FROM_POINTER[0] - width;
            y = py + FROM_POINTER[1];
            if (y + height > bottom)
                y = py - FROM_POINTER[1] - height;
        } else if (anchor instanceof Clutter.Actor && anchor.has_allocation() && anchor.width > 0) {
            const box = anchor.get_transformed_extents();
            x = box.get_x() + (box.get_width() - width) / 2;
            y = box.get_y() + box.get_height() + GAP;
            if (y + height > bottom)
                y = box.get_y() - GAP - height;
        }
        // No anchor, or one not laid out yet: the top of the screen, under the bar
        if (!Number.isFinite(x) || !Number.isFinite(y)) {
            x = monitor.x + (monitor.width - width) / 2;
            y = monitor.y + Main.layoutManager.panelBox.height + GAP;
        }
        x = Math.max(left, Math.min(x, right - width));
        y = Math.max(top, Math.min(y, bottom - height));
        card.set_position(Math.round(x), Math.round(y));
    }

    _gotIt() {
        const card = this._card;
        if (!card)
            return;
        const [x, y] = card.get_position();
        this._close(false);
        const pill = new St.Label({
            style_class: 'amethystora-hints-pill',
            text: _('Recorded: this tip will not come back.'),
            opacity: 0,
        });
        pill.clutter_text.use_markup = false;
        pill.accessible_role = Atk.Role.NOTIFICATION;
        Main.layoutManager.addTopChrome(pill);
        pill.set_position(x, y + 8);
        this._pill = pill;
        pill.ease({opacity: 255, duration: 200, mode: Clutter.AnimationMode.EASE_OUT_QUAD});
        this._pillTimeout = GLib.timeout_add(GLib.PRIORITY_DEFAULT, RECORDED_FOR, () => {
            this._pillTimeout = 0;
            this._fade(pill, () => {
                if (this._pill === pill)
                    this._pill = null;
                this._glass();
            });
            return GLib.SOURCE_REMOVE;
        });
        this._glass();
    }

    _fade(actor, done) {
        actor.reactive = false;
        actor.ease({
            opacity: 0,
            duration: 150,
            mode: Clutter.AnimationMode.EASE_OUT_QUAD,
            onStopped: () => {
                gone(actor);
                done();
            },
        });
    }

    // The card goes (faded, or at once when the module stops); with all, the pill too
    _close(all) {
        if (this._timeout)
            GLib.source_remove(this._timeout);
        this._timeout = 0;
        global.display.disconnectObject(this);
        Main.overview.disconnectObject(this);
        const card = this._card;
        this._card = null;
        if (card) {
            if (all) {
                gone(card);
            } else {
                card.remove_all_transitions();
                this._fade(card, () => this._glass());
            }
        }
        if (all) {
            if (this._pillTimeout)
                GLib.source_remove(this._pillTimeout);
            this._pillTimeout = 0;
            const pill = this._pill;
            this._pill = null;
            if (pill)
                gone(pill);
        }
        this._glass();
    }

    glassSurfaces() {
        const surfaces = [];
        if (this._card)
            surfaces.push({actor: this._card, material: 'glass', radius: 18});
        if (this._pill)
            surfaces.push({actor: this._pill, material: 'glass', radius: null});
        return surfaces;
    }

    _glass() {
        this._ext.modules?.get('glass')?.surfacesChanged?.();
    }

    // --- This module's own tips --------------------------------------------------------------------------------------

    _watchOwn() {
        this._own = {};
        // Super+Space: apps started from a later page of the app grid, counted only while the overview is up
        if (!this.wasShown('search')) {
            const apps = Shell.AppSystem.get_default();
            Main.overview.connectObject(
                'showing', () => {
                    apps.disconnectObject(this._own);
                    apps.connectObject('app-state-changed', (system, app) => {
                        if (app.state === Shell.AppState.STARTING && this._gridPage() > 0)
                            this._searches++;
                    }, this._own);
                },
                'hidden', () => {
                    apps.disconnectObject(this._own);
                    if (this.wasShown('search'))
                        this._unwatch('search');
                    else if (this._searches >= SEARCH_AFTER)
                        this.offerWhenClosed('search', {text: _('Tip: Super+Space searches apps, files and settings.')});
                },
                this._own);
        }
        // Files, the first time it has the focus
        if (!this.wasShown('folder-colour')) {
            global.display.connectObject('notify::focus-window', () => {
                if (global.display.focus_window?.get_wm_class() !== FILES)
                    return;
                const text = _('Tip: right-click a folder, then Scripts, to give it a colour or go back in time.');
                if (this.offer('folder-colour', {text}) || this.wasShown('folder-colour'))
                    this._unwatch('folder-colour');
            }, this._own);
        }
    }

    // The page the app grid shows, or 0 when it is not the app grid
    _gridPage() {
        const controls = Main.overview._overview?.controls;
        const appDisplay = controls?._appDisplay;
        if (!appDisplay?.visible || !controls._stateAdjustment || controls._stateAdjustment.value < 1.5)
            return 0;
        return appDisplay._grid?.currentPage ?? 0;
    }

    _unwatch(id) {
        if (!this._own)
            return;
        if (id === 'folder-colour')
            global.display.disconnectObject(this._own);
        if (id === 'search') {
            Main.overview.disconnectObject(this._own);
            Shell.AppSystem.get_default().disconnectObject(this._own);
        }
    }

    _unwatchOwn() {
        if (!this._own)
            return;
        global.display.disconnectObject(this._own);
        Main.overview.disconnectObject(this._own);
        Shell.AppSystem.get_default().disconnectObject(this._own);
        this._own = null;
    }
}
