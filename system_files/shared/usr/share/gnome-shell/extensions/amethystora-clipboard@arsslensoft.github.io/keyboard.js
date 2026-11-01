import Clutter from 'gi://Clutter';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';

export class Keyboard {
    #device;
    #contentPurpose;

    constructor () {
        // The stage's context from GNOME 47, the only way there is in GNOME 51, which has no default backend
        let seat = (global.stage.context?.get_backend() ?? Clutter.get_default_backend()).get_default_seat();
        this.#device = seat.create_virtual_device(Clutter.InputDeviceType.KEYBOARD_DEVICE);

        Main.inputMethod.connectObject('notify::content-purpose', (method) => {
            this.#contentPurpose = method.content_purpose;
        }, this);
    }

    destroy () {
        Main.inputMethod.disconnectObject(this);
        this.#device.run_dispose();
    }

    #notify (key, state) {
        this.#device.notify_keyval(
            Clutter.get_current_event_time() * 1000,
            key,
            state
        );
    }

    get purpose () {
        return this.#contentPurpose;
    }

    press (key) {
        this.#notify(key, Clutter.KeyState.PRESSED);
    }

    release (key) {
        this.#notify(key, Clutter.KeyState.RELEASED);
    }
}
