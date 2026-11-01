// Three changes to GNOME Shell itself, the ones the image has always made: its animations take a share of their time
// (0.6 unless the person chose another speed, and half of that while the power saver or the light desktop is on),
// switching workspaces shows no popup in the middle of the screen (the workspaces in the top bar say where you are), and
// the Activities button is gone from the top bar (the logo menu and the workspaces stand there).
//
// The speed is the motion's `speed` key (ame desktop animation-speed, Control): snappy, normal, relaxed, or off, which
// asks St to hold every animation back, the way GNOME itself does while the screen is shared remotely. GNOME's own
// switch for animations is the person's, in Settings, and is never touched: with it off, every animation is already
// instant, whatever the speed says.

import St from 'gi://St';

import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import * as WorkspaceSwitcherPopup from 'resource:///org/gnome/shell/ui/workspaceSwitcherPopup.js';

// The share of its usual time an animation takes at each speed, and the share of that it takes while the power saver
// is on (the comfort module watches the profile). This module is the only one that sets the factor.
const MOTION = 'org.gnome.shell.extensions.amethystora-ui.motion';
export const SPEEDS = {snappy: 0.4, normal: 0.6, relaxed: 0.9};
const DEFAULT_SPEED = 'normal';
const POWER_SAVER = 0.5;

export class Tweaks {
    constructor(ext) {
        this._ext = ext;
    }

    enable() {
        const settings = St.Settings.get();
        this._factor = settings.slow_down_factor;
        this._inhibited = false;
        this._motion = this._ext.getSettings(MOTION);
        this._motion.connectObject('changed::speed', () => this._speed(), this);
        this._unlisten = this._ext.modules.get('comfort')?.listen(() => this._speed()) ?? null;
        this._speed();

        // The popup is made hidden and only shown by display(), so it goes before it is ever drawn. Destroying it
        // is also what gives the window manager back the workspace updates it held for it.
        const popup = WorkspaceSwitcherPopup.WorkspaceSwitcherPopup.prototype;
        this._display = popup.display;
        popup.display = function () {
            this.destroy();
        };

        // The button's container rather than the button, and again whenever anything shows it
        this._activities = Main.panel.statusArea.activities?.container ?? null;
        if (this._activities) {
            this._activities.hide();
            this._activities.connectObject('notify::visible', actor => {
                if (actor.visible)
                    actor.hide();
            }, this);
        }
    }

    disable() {
        this._unlisten?.();
        this._unlisten = null;
        this._motion?.disconnectObject(this);
        // Disposed now rather than by the collector (dock.js says why)
        this._motion?.run_dispose();
        this._motion = null;
        this._hold(false);
        if (this._factor !== undefined)
            St.Settings.get().slow_down_factor = this._factor;
        this._factor = undefined;

        if (this._display)
            WorkspaceSwitcherPopup.WorkspaceSwitcherPopup.prototype.display = this._display;
        this._display = null;

        if (this._activities) {
            this._activities.disconnectObject(this);
            // The lock screen's panel has no Activities button to give back
            if (!Main.sessionMode.isLocked)
                this._activities.show();
        }
        this._activities = null;
    }

    // The speed chosen, as its name
    get speed() {
        const name = this._motion?.get_string('speed') ?? DEFAULT_SPEED;
        return name === 'off' || SPEEDS[name] ? name : DEFAULT_SPEED;
    }

    _speed() {
        if (!this._motion)
            return;
        const name = this.speed;
        this._hold(name === 'off');
        // The power saver, or the light desktop, or a focus profile asking for what they do
        const comfort = this._ext.modules.get('comfort');
        const saver = comfort?.lighter ?? comfort?.powerSaver ?? false;
        const factor = SPEEDS[name] ?? SPEEDS[DEFAULT_SPEED];
        St.Settings.get().slow_down_factor = saver ? factor * POWER_SAVER : factor;
    }

    // St counts the holds on animations: this module keeps at most one
    _hold(on) {
        if (on === this._inhibited)
            return;
        this._inhibited = on;
        if (on)
            St.Settings.get().inhibit_animations();
        else
            St.Settings.get().uninhibit_animations();
    }
}
