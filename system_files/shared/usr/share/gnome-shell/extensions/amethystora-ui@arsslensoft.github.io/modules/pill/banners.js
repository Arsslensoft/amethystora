// Banners that grow out of the pill and go back in (R7-17, the key weight): GNOME's own banner, with its own slide and
// its own timing, and a scale from the top of its middle, under the pill. GNOME's function runs first and untouched, and
// the scale rides on what it started. A banner placed at another edge, or a pill that is not in the middle, keeps GNOME's
// motion; so do reduce motion and a slow computer.

import Clutter from 'gi://Clutter';

import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import * as MessageTray from 'resource:///org/gnome/shell/ui/messageTray.js';

// How small a banner starts and ends
const SMALL = 0.3;
// GNOME's own time for a banner to show (messageTray.js ANIMATION_TIME)
const DURATION = 200;

export class Banners {
    constructor(wanted) {
        this._wanted = wanted;
    }

    enable() {
        const tray = Main.messageTray;
        if (typeof tray._updateShowingNotification !== 'function' || typeof tray._hideNotification !== 'function')
            return;
        const self = this;
        this._show = tray._updateShowingNotification;
        this._hide = tray._hideNotification;
        tray._updateShowingNotification = function (...args) {
            // GNOME calls this for a banner that is updated while it shows as well, where its own dip is small: the banner
            // grows only when it comes in, from above the screen (51 messageTray.js _showNotification sets that y)
            const coming = this._bannerBin.y < 0;
            const result = self._show.apply(this, args);
            if (coming)
                self._grow();
            return result;
        };
        tray._hideNotification = function (animate, ...rest) {
            const result = self._hide.call(this, animate, ...rest);
            if (animate)
                self._shrink();
            return result;
        };
    }

    disable() {
        if (!this._show)
            return;
        // The instance's own functions go: its prototype's are GNOME's, and the hold's wrap of them stays as it is
        delete Main.messageTray._updateShowingNotification;
        delete Main.messageTray._hideNotification;
        this._show = this._hide = null;
    }

    _inMiddle() {
        return this._wanted() && Main.messageTray.bannerAlignment === Clutter.ActorAlign.CENTER;
    }

    // GNOME has started its slide from just above: the scale it began with is made smaller, about the top
    _grow() {
        if (!this._inMiddle())
            return;
        const bin = Main.messageTray._bannerBin;
        // GNOME's own end hangs on its slide: with no slide under way, its motion is left alone
        if (!bin.get_transition('y'))
            return;
        bin.set_pivot_point(0.5, 0);
        bin.remove_transition('scale-x');
        bin.remove_transition('scale-y');
        bin.scale_x = SMALL;
        bin.scale_y = SMALL;
        bin.ease({scale_x: 1, scale_y: 1, duration: DURATION, mode: Clutter.AnimationMode.EASE_OUT_BACK});
    }

    _shrink() {
        if (!this._inMiddle())
            return;
        const bin = Main.messageTray._bannerBin;
        bin.set_pivot_point(0.5, 0);
        bin.ease({scale_x: SMALL, scale_y: SMALL, duration: DURATION, mode: Clutter.AnimationMode.EASE_IN_QUAD});
    }
}
