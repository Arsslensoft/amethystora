// Clear voice: a switch at the end of GNOME's microphone menu, which GNOME shows only while an app records. On, the
// microphone goes through PipeWire's own echo cancel and noise suppression as an input of its own, made the one in use;
// the calls hear the voice without the fan, the keyboard or the room's echo, in every app.
//
// The switch shows the key clear-voice and turns it with `amethystora-sound clear-voice on|off`, which starts or stops
// the PipeWire unit and moves the input in use; what the switch then shows is the key, never a guess. It is built while
// the menu is open and taken down when it closes.

import Gio from 'gi://Gio';

import {gettext as _} from 'resource:///org/gnome/shell/extensions/extension.js';

import {SwitchRow, inputSlider, keepMenu, runHelper, whenSlider} from './menu.js';

export class Voice {
    constructor(settings) {
        this._settings = settings;
    }

    enable() {
        this._cancellable = new Gio.Cancellable();
        this._settings.connectObject('changed::clear-voice', () => this._sync(), this);
        this._unwait = whenSlider(inputSlider, slider => this._attach(slider));
    }

    _attach(slider) {
        this._slider = slider;
        this._release = keepMenu(this._slider, () => true);
        this._slider.menu.connectObject('open-state-changed', (menu, open) => (open ? this._open() : this._close()), this);
        if (this._slider.menu.isOpen)
            this._open();
    }

    disable() {
        this._unwait?.();
        this._unwait = null;
        this._cancellable?.cancel();
        this._cancellable = null;
        this._settings.disconnectObject(this);
        this._slider?.menu.disconnectObject(this);
        this._close();
        this._release?.();
        this._release = null;
        this._slider = null;
    }

    _open() {
        if (this._row)
            return;
        this._row = new SwitchRow(_('Clear voice'), _('Takes out background noise and echo'), () => this.toggle());
        // After GNOME's list of inputs, before its separator and Sound Settings
        this._slider.menu.addMenuItem(this._row, 1);
        this._failed = false;
        this._sync();
    }

    _close() {
        this._row?.destroy();
        this._row = null;
    }

    _sync() {
        if (!this._row || this._pending)
            return;
        this._row.state = this._settings.get_boolean('clear-voice');
        // What went wrong stays in sight under the switch, in place of what it does
        this._row.text = this._failed ? _('Clear voice could not be changed.')
            : _('Takes out background noise and echo');
    }

    // The person's switch: the helper does it, and the key it writes is what the switch then shows
    toggle() {
        if (this._pending)
            return Promise.resolve(false);
        const want = !this._settings.get_boolean('clear-voice');
        this._pending = true;
        if (this._row)
            this._row.state = want;
        this._failed = false;
        return runHelper(['clear-voice', want ? 'on' : 'off'], this._cancellable).then(ok => {
            this._pending = false;
            this._failed = !ok;
            this._sync();
            return ok;
        });
    }

    // --- For the suites ---------------------------------------------------------------------------------------------

    state() {
        return {
            key: this._settings.get_boolean('clear-voice'),
            shown: !!this._row,
            switch: this._row?.state ?? null,
            error: this._failed ?? false,
            menuEnabled: this._slider?.menuEnabled ?? null,
        };
    }
}
