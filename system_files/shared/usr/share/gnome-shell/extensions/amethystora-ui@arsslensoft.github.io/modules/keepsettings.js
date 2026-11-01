// GJS sweeping a dropped Gio.Settings while dconf's worker thread dispatches a change to it deadlocks GNOME Shell for good
// (GJS's toggle queue against the settings backend's lock): the screen stays as it was until the session is killed. Our
// own code disposes every one it makes (build_files/shared/test-settings-disposed.mjs), but GNOME Shell's own code drops
// some (each background, a popup menu's switch, a swipe tracker), and a burst of changes, as a theme switch makes, finds
// them. So none made from here on is left to the collector: each is held until it is disposed, which costs a few small
// objects a session. At module scope and never undone: a screen lock disables the extension, and this has to outlast it.

import Gio from 'gi://Gio';

const kept = new Set();
const init = Gio.Settings.prototype._init;
const dispose = Gio.Settings.prototype.run_dispose;

Gio.Settings.prototype._init = function (...args) {
    init.apply(this, args);
    kept.add(this);
};

Gio.Settings.prototype.run_dispose = function () {
    kept.delete(this);
    return dispose.call(this);
};
