// Volume and brightness as a pill under the top bar: every on-screen display GNOME shows (volume, brightness, the
// keyboard's light, and the ones with only a word, such as Caps Lock) is drawn as a slim round bar near the top of its
// screen instead of a card low in the middle, for GNOME's own time. GNOME decides when one shows; this only moves it.

import Clutter from 'gi://Clutter';
import St from 'gi://St';

import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import * as OsdWindow from 'resource:///org/gnome/shell/ui/osdWindow.js';

const CLASS = 'amethystora-pill-osd';
// Its top's distance below the top of the screen's work area (logical px)
const GAP = 8;

function place(osd) {
    osd._hbox?.add_style_class_name(CLASS);
    osd.y_align = Clutter.ActorAlign.START;
    const index = osd._monitorIndex ?? Main.layoutManager.primaryIndex;
    const monitor = Main.layoutManager.monitors[index];
    if (!monitor)
        return;
    const area = Main.layoutManager.getWorkAreaForMonitor(index);
    const scale = St.ThemeContext.get_for_stage(global.stage).scale_factor;
    // A translation, not a margin: St writes a margin back from the stylesheet on every change of style
    osd.translation_y = area.y - monitor.y + GAP * scale;
}

function restore(osd) {
    osd._hbox?.remove_style_class_name(CLASS);
    osd.y_align = Clutter.ActorAlign.END;
    osd.translation_y = 0;
}

export class Osd {
    enable() {
        const proto = OsdWindow.OsdWindow.prototype;
        this._original = proto.show;
        const original = this._original;
        proto.show = function (...args) {
            place(this);
            return original.apply(this, args);
        };
    }

    disable() {
        if (this._original)
            OsdWindow.OsdWindow.prototype.show = this._original;
        this._original = null;
        for (const osd of Main.osdWindowManager?._osdWindows ?? [])
            restore(osd);
    }

    // Where the first one shown is, for the suites
    get state() {
        const osd = (Main.osdWindowManager?._osdWindows ?? []).find(o => o.visible) ?? Main.osdWindowManager?._osdWindows?.[0];
        const box = osd?._hbox;
        if (!box)
            return null;
        const [x, y] = box.get_transformed_position();
        const [width, height] = box.get_transformed_size();
        return {x, y, width, height, class: box.has_style_class_name(CLASS), visible: !!osd.visible, yAlign: osd.y_align, translationY: osd.translation_y};
    }
}
