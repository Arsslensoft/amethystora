// A plugin's badge on a pinned app (R7-07): its printed text, when it is a whole number, is the count in the corner, and its progress
// is the bar along the bottom, drawn by the dock as an app's own would be (dock/launcher.js). Only an app this machine has gets one, and
// a number is the only text that is read as a count: nothing a plugin writes is drawn as anything but a number there.

import {dockApp, number} from './clamp.js';

export class DockBadges {
    constructor(launcher) {
        this._launcher = launcher;
        this._owners = new Set();
    }

    // plugins: the clamped plugins that name a dock place; the rest are cleared
    sync(plugins) {
        const wanted = new Set();
        for (const plugin of plugins) {
            const out = plugin.output ?? plugin.sample;
            const app = plugin.places.dock?.app;
            if (!dockApp(app) || !out || out.hidden || plugin.paused || out.private)
                continue;
            wanted.add(plugin.id);
            const count = /^\d{1,6}$/.test(out.text) ? Number(out.text) : 0;
            this._launcher.setPlugin(plugin.id, app, {count, progress: number(out.progress, 0, 1, null)});
        }
        for (const id of this._owners) {
            if (!wanted.has(id))
                this._launcher.setPlugin(id, '', null);
        }
        this._owners = wanted;
    }

    clear() {
        for (const id of this._owners)
            this._launcher.setPlugin(id, '', null);
        this._owners.clear();
    }
}
