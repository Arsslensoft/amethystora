// Sound that looks after itself: the players pause when the headphones come off (modules/audio/unplug.js), Clear voice
// as a switch in GNOME's microphone menu (modules/audio/voice.js) and a preset per output in GNOME's output menu
// (modules/audio/presets.js).
//
// The keys of org.gnome.shell.extensions.amethystora-ui.audio, read live: pause-on-unplug (on), clear-voice and
// sound-presets (off). The sound itself is changed by /usr/libexec/amethystora-sound, as the account, with PipeWire's
// own modules in user units of their own; the shell only shows the switch and the row and runs that helper with an
// argument vector. A part that is off has nothing connected and nothing made; the two menus' parts are built while
// their menu is open and taken down when it closes.
//
// For the suites: unplug, voice and presets are the three parts, each with its state() and test helpers.

import {Presets} from './audio/presets.js';
import {Unplug} from './audio/unplug.js';
import {Voice} from './audio/voice.js';

const SCHEMA = 'org.gnome.shell.extensions.amethystora-ui.audio';

export class Audio {
    constructor(ext) {
        this._ext = ext;
    }

    enable() {
        this._settings = this._ext.getSettings(SCHEMA);
        this._settings.connectObject(
            'changed::pause-on-unplug', () => this._sync(),
            this);
        this._sync();
        // The microphone's switch is always offered (GNOME shows that menu only while an app records); the presets'
        // row only while the presets are on, which the part reads itself
        this._start('_voice', () => new Voice(this._settings));
        this._start('_presets', () => new Presets(this._settings));
    }

    disable() {
        this._settings?.disconnectObject(this);
        this._stop('_presets');
        this._stop('_voice');
        this._stop('_unplug');
        // Disposed now rather than by the collector (dock.js says why)
        this._settings?.run_dispose();
        this._settings = null;
    }

    _sync() {
        const wanted = this._settings.get_boolean('pause-on-unplug');
        if (wanted && !this._unplug)
            this._start('_unplug', () => new Unplug());
        else if (!wanted && this._unplug)
            this._stop('_unplug');
    }

    _start(name, make) {
        this[name] = make();
        try {
            this[name].enable();
        } catch (e) {
            console.error(`Amethystora UI: the audio module's ${name.slice(1)} did not start: ${e.message}\n${e.stack}`);
            this._stop(name);
        }
    }

    _stop(name) {
        const part = this[name];
        this[name] = null;
        try {
            part?.disable();
        } catch (e) {
            console.error(`Amethystora UI: the audio module's ${name.slice(1)} did not stop cleanly: ${e.message}`);
        }
    }

    get unplug() {
        return this._unplug;
    }

    get voice() {
        return this._voice;
    }

    get presets() {
        return this._presets;
    }
}
