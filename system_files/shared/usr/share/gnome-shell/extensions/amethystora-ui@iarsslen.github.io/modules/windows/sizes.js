// A size for recording: the window menu's Size for recording makes a window exactly 1280 × 720, 1920 × 1080, square or
// upright, centred on its screen, for screencasts, tutorials and screenshots that line up. The size is in the screen's
// own pixels, so a recording of the window has it whatever the screen's scale. In the classic layout, and for the
// tiling's floating windows (a window on the strip has the strip's size). It only runs when chosen.

import Meta from 'gi://Meta';

import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import * as PopupMenu from 'resource:///org/gnome/shell/ui/popupMenu.js';
import {gettext as _} from 'resource:///org/gnome/shell/extensions/extension.js';

export const SIZES = [[1280, 720], [1920, 1080], [1080, 1080], [1080, 1920]];

export class Sizes {
    constructor(module) {
        this._module = module;
        this._ext = module.ext;
    }

    enable() {}

    disable() {}

    // Whether a window takes a size from here: one the person sizes, not the strip
    _sizable(window) {
        if (window.get_window_type() !== Meta.WindowType.NORMAL || window.is_fullscreen() || !window.resizeable ||
            this._module.holds(window))
            return false;
        if (this._module.classic)
            return true;
        const engine = this._ext.modules?.get('tiling')?.engine;
        return !!engine && !engine.isTiled(window);
    }

    // The frame, in the stage's pixels, that is `width` × `height` of the screen's own, centred in its work area; null
    // where it does not fit
    frameFor(window, width, height) {
        const monitor = window.get_monitor();
        if (monitor < 0)
            return null;
        const factor = global.display.get_monitor_scale(monitor) || 1;
        const area = Main.layoutManager.getWorkAreaForMonitor(monitor);
        const w = Math.round(width / factor);
        const h = Math.round(height / factor);
        if (w > area.width || h > area.height)
            return null;
        return {x: area.x + Math.round((area.width - w) / 2), y: area.y + Math.round((area.height - h) / 2),
            width: w, height: h};
    }

    resize(window, width, height) {
        const rect = this.frameFor(window, width, height);
        if (!rect)
            return false;
        if (window.maximized_horizontally || window.maximized_vertically)
            window.unmaximize();
        window.move_resize_frame(true, rect.x, rect.y, rect.width, rect.height);
        return true;
    }

    menuItems(menu, window) {
        if (!this._sizable(window))
            return null;
        const sub = new PopupMenu.PopupSubMenuMenuItem(_('Size for recording'));
        menu.addMenuItem(sub);
        for (const [width, height] of SIZES) {
            // Written as screen sizes are, without a thousands separator
            const words = _('%d × %d').format(width, height);
            const item = sub.menu.addAction(words, () => this.resize(window, width, height));
            if (!this.frameFor(window, width, height))
                item.setSensitive(false);
        }
        return sub;
    }
}
