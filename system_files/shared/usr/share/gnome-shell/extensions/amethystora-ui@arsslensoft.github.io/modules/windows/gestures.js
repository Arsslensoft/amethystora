// Touchpad gestures beside GNOME's own: a three-finger pinch opens the app grid, three fingers spread show the
// desktop, three fingers down fan the windows of the app in use out from the dock, and four fingers to the left open
// Today and the notifications (to the right close them). Two fingers stay the apps' own; three fingers up and sideways
// stay GNOME's. A gesture is taken only once it is recognised, so everything before that still reaches GNOME.

import Clutter from 'gi://Clutter';
import Gio from 'gi://Gio';
import Shell from 'gi://Shell';

import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import {gettext as _} from 'resource:///org/gnome/shell/extensions/extension.js';

// GNOME's own distance before a swipe has a direction
const THRESHOLD = 16;
const PINCH_IN = 0.92;
const SPREAD_OUT = 1.08;
const DOWN_FIRES = 60;
const TODAY_FIRES = 80;
const GRID_TIME = 250;

const PHASES = {
    [Clutter.TouchpadGesturePhase.BEGIN]: 'begin',
    [Clutter.TouchpadGesturePhase.UPDATE]: 'update',
    [Clutter.TouchpadGesturePhase.END]: 'end',
    [Clutter.TouchpadGesturePhase.CANCEL]: 'cancel',
};

const clamp = (v, a, b) => Math.max(a, Math.min(b, v));

// Stands in for GNOME's swipe tracker when the pinch drives the overview: the pinch has its own distances
const STAND_IN = {confirmSwipe() {}};

export class Gestures {
    constructor(module) {
        this._module = module;
        this._ext = module.ext;
        this.connected = false;
        this.lastResult = null;
    }

    enable() {
        this._touchpad = new Gio.Settings({schema_id: 'org.gnome.desktop.peripherals.touchpad'});
        global.stage.connectObject('captured-event::touchpad', (stage, event) => this._event(event), this);
        this.connected = true;
        this._seq = null;
        // The tip (the hints module): the app grid opened with the pointer on a machine with a touchpad
        if (!this._ext.modules?.get('hints')?.wasShown?.('pinch')) {
            this._tipOwner = {};
            Main.overview.dash.showAppsButton.connectObject('notify::checked', () => this._gridOpened(), this._tipOwner);
        }
    }

    disable() {
        this._stopTip();
        global.stage.disconnectObject(this);
        this.connected = false;
        // A pinch under way leaves the overview where it was going back to
        if (this._seq?.state === 'grid')
            this._endGrid(false);
        if (this._seq?.state === 'spread')
            this._module.desktop.end(false);
        this._seq = null;
        // Disposed now rather than by the collector (dock.js says why)
        this._touchpad?.run_dispose();
        this._touchpad = null;
    }

    _key(name) {
        return this._module.settings.get_boolean(`gesture-${name}`);
    }

    // The button was clicked (not a key, not a gesture), the pinch is on and a touchpad is there: the tip comes once
    // the overview has closed, since none shows over it
    _gridOpened() {
        const button = Main.overview.dash.showAppsButton;
        const event = Clutter.get_current_event();
        const clicked = [Clutter.EventType.BUTTON_PRESS, Clutter.EventType.BUTTON_RELEASE].includes(event?.type());
        if (!button.checked || !clicked || !this._key('pinch'))
            return;
        const hints = this._ext.modules?.get('hints');
        if (hints?.wasShown('pinch')) {
            this._stopTip();
            return;
        }
        const seat = global.stage.context.get_backend().get_default_seat();
        if (seat.list_devices().some(device => device.get_device_type() === Clutter.InputDeviceType.TOUCHPAD_DEVICE))
            hints?.offerWhenClosed('pinch', {text: _('Tip: pinch with three fingers on the touchpad to see your apps.')});
    }

    _stopTip() {
        if (!this._tipOwner)
            return;
        Main.overview.dash.showAppsButton.disconnectObject(this._tipOwner);
        this._tipOwner = null;
    }

    _event(event) {
        const type = event.type();
        let kind;
        if (type === Clutter.EventType.TOUCHPAD_PINCH)
            kind = 'pinch';
        else if (type === Clutter.EventType.TOUCHPAD_SWIPE)
            kind = 'swipe';
        else
            return Clutter.EVENT_PROPAGATE;
        const [dx, dy] = event.get_gesture_motion_delta_unaccelerated();
        return this.feed({
            kind, fingers: event.get_touchpad_gesture_finger_count(), phase: PHASES[event.get_gesture_phase()],
            dx, dy, scale: kind === 'pinch' ? event.get_gesture_pinch_scale() : 1,
        });
    }

    // The recogniser: what the event handler gives it for each touchpad event, as plain numbers. Returns whether the
    // event is taken (Clutter.EVENT_STOP) or goes on to GNOME.
    feed(step) {
        if (Array.isArray(step))
            return step.map(s => this.feed(s));
        this._module.count('gestureEvent');
        const {kind, fingers, phase} = step;
        if (phase === 'begin') {
            this._seq = {kind, fingers, state: 'pending', x: 0, y: 0, scale: 1};
            return Clutter.EVENT_PROPAGATE;
        }
        const seq = this._seq;
        if (!seq || seq.kind !== kind)
            return Clutter.EVENT_PROPAGATE;
        seq.x += step.dx ?? 0;
        seq.y += step.dy ?? 0;
        if (kind === 'pinch')
            seq.scale = step.scale ?? seq.scale;
        if (seq.state === 'pending' && phase === 'update')
            this._recognise(seq);
        if (seq.state === 'pending' || seq.state === 'ignored') {
            if (phase !== 'update')
                this._seq = null;
            return Clutter.EVENT_PROPAGATE;
        }
        if (phase === 'update')
            this._follow(seq);
        else
            this._finish(seq, phase === 'end');
        if (phase !== 'update')
            this._seq = null;
        return Clutter.EVENT_STOP;
    }

    _natural() {
        return this._touchpad?.get_boolean('natural-scroll') ?? true;
    }

    _tiling() {
        return this._ext.modules?.get('tiling')?.settings ?? null;
    }

    _recognise(seq) {
        const normal = Main.actionMode === Shell.ActionMode.NORMAL;
        if (seq.kind === 'pinch') {
            if (seq.fingers !== 3)
                return;
            if (seq.scale <= PINCH_IN) {
                if (!this._key('pinch') || Main.overview.visible || !normal) {
                    seq.state = 'ignored';
                } else if (this._module.desktop.out) {
                    // Windows that are out come back instead
                    seq.state = 'back';
                    this._module.desktop.bringBack();
                    this.lastResult = 'back';
                } else {
                    seq.state = 'grid';
                    Main.overview._gestureBegin(STAND_IN);
                    this._follow(seq);
                }
            } else if (seq.scale >= SPREAD_OUT) {
                if (!this._key('spread') || !normal || !this._module.classic || !this._module.desktop.begin())
                    seq.state = 'ignored';
                else
                    seq.state = 'spread';
            }
            return;
        }
        if (Math.hypot(seq.x, seq.y) < THRESHOLD)
            return;
        const sign = this._natural() ? 1 : -1;
        const tiling = this._tiling();
        const tilingOn = tiling?.get_boolean('enabled');
        if (seq.fingers === 3) {
            const down = Math.abs(seq.y) > Math.abs(seq.x) && sign * seq.y > 0;
            // The tiling's pile of workspaces takes three fingers down when it is set to
            const pile = tilingOn && tiling.get_boolean('gesture-enabled') &&
                tiling.get_int('gesture-workspace-fingers') === 3;
            seq.state = down && this._key('down') && normal && !Main.overview.visible && !pile ? 'down' : 'ignored';
        } else if (seq.fingers === 4) {
            const across = Math.abs(seq.x) > Math.abs(seq.y);
            const taken = tilingOn && tiling.get_boolean('gesture-enabled') &&
                (tiling.get_int('gesture-horizontal-fingers') === 4 || tiling.get_int('gesture-workspace-fingers') === 4);
            // Today opens from the desktop or the overview, and closes from its own menu
            const modes = normal || Main.actionMode === Shell.ActionMode.OVERVIEW ||
                !!Main.panel.statusArea.dateMenu?.menu?.isOpen;
            seq.state = across && this._key('today') && modes && !taken ? 'today' : 'ignored';
        } else {
            seq.state = 'ignored';
        }
    }

    _follow(seq) {
        if (seq.state === 'grid') {
            const progress = clamp((1 - seq.scale) / 0.5, 0, 1) * 2;
            Main.overview._gestureUpdate(STAND_IN, Math.max(progress, 0.001));
        } else if (seq.state === 'spread') {
            this._module.desktop.progress(clamp((seq.scale - 1) / 0.5, 0, 1));
        }
    }

    _finish(seq, ended) {
        const sign = this._natural() ? 1 : -1;
        switch (seq.state) {
        case 'grid':
            this._endGrid(ended && clamp((1 - seq.scale) / 0.5, 0, 1) * 2 >= 1);
            break;
        case 'spread':
            this._module.desktop.end(ended && clamp((seq.scale - 1) / 0.5, 0, 1) >= 0.5);
            this.lastResult = ended ? 'spread' : null;
            break;
        case 'down':
            if (ended && sign * seq.y >= DOWN_FIRES)
                this.lastResult = this._fan() ? 'fan' : 'down-nothing';
            break;
        case 'today': {
            if (!ended || Math.abs(seq.x) < TODAY_FIRES)
                break;
            const menu = Main.panel.statusArea.dateMenu?.menu;
            const left = sign * seq.x < 0;
            if (left && !menu?.isOpen)
                Main.panel.toggleCalendar();
            else if (!left && menu?.isOpen)
                menu.close();
            this.lastResult = left ? 'today' : 'today-closed';
            break;
        }
        }
    }

    _endGrid(toGrid) {
        Main.overview._gestureEnd(STAND_IN, GRID_TIME, toGrid ? 2 : 0);
        this.lastResult = toGrid ? 'grid' : 'grid-back';
    }

    // The windows of the app in use, fanned out from its place in the dock
    _fan() {
        const window = global.display.focus_window;
        const app = window ? Shell.WindowTracker.get_default().get_window_app(window) : null;
        if (!app || app.get_windows().length < 2)
            return false;
        const dock = this._ext.modules?.get('dock');
        if (!dock)
            return false;
        if (typeof dock.showWindows === 'function')
            return dock.showWindows(app) !== false;
        // The dock opens its fan from an app's icon, where a right click would
        const icon = this._dockIcon(app, window.get_monitor());
        if (!icon)
            return false;
        icon.popupMenu();
        return true;
    }

    _dockIcon(app, monitor) {
        const found = [];
        const walk = (actor, depth, inDock) => {
            if (depth > 9)
                return;
            const dock = inDock || actor.name === 'amethystoraDock';
            if (dock && actor.app === app && typeof actor.popupMenu === 'function' && actor.mapped)
                found.push(actor);
            for (const child of actor.get_children())
                walk(child, depth + 1, dock);
        };
        walk(Main.layoutManager.uiGroup, 0, false);
        return found.find(icon => {
            const [x, y] = icon.get_transformed_position();
            const m = Main.layoutManager.monitors[monitor];
            return m && x >= m.x && x < m.x + m.width && y >= m.y && y < m.y + m.height;
        }) ?? found[0] ?? null;
    }
}
