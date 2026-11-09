// Trim a recording (R6-19, offered, off: recording-trim). GNOME's notice that a screen recording was saved gains Trim,
// which opens the recording in the Amethystora apps' small trimming window (amethystora-trim): drag its two ends, then
// save that part as WebM or a short GIF beside it. The recording itself is never changed.
//
// GNOME says a recording was saved from ScreenshotUI.stopScreencast, through its _showNotification (screenshot.js of
// GNOME 50 and 51 alike): while the key is on, both are wrapped on GNOME's one screenshot UI, so that the notice shown
// while a recording stops, and only that one, gets the button. Its path is the one GNOME recorded to, handed to the
// window as one argument.

import Gio from 'gi://Gio';

import {gettext as _} from 'resource:///org/gnome/shell/extensions/extension.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import * as MessageTray from 'resource:///org/gnome/shell/ui/messageTray.js';

const KEY = 'recording-trim';
const TRIM = '/usr/bin/amethystora-trim';

export class RecordingTrim {
    constructor(ui, settings) {
        this._ui = ui;
        this._settings = settings;
    }

    enable() {
        this._settings.connectObject(`changed::${KEY}`, () => this._sync(), this);
        this._sync();
    }

    disable() {
        this._settings?.disconnectObject(this);
        this._unwrap();
    }

    _sync() {
        if (this._settings.get_boolean(KEY))
            this._wrap();
        else
            this._unwrap();
    }

    _wrap() {
        const ui = this._ui;
        if (this._wrapped || typeof ui?.stopScreencast !== 'function' || typeof ui._showNotification !== 'function')
            return;
        this._wrapped = true;
        this._stopDescriptor = Object.getOwnPropertyDescriptor(ui, 'stopScreencast');
        this._showDescriptor = Object.getOwnPropertyDescriptor(ui, '_showNotification');
        const stop = ui.stopScreencast;
        const show = ui._showNotification;
        const trim = this;
        ui.stopScreencast = async function (...args) {
            trim._stopping = this._screencastPath ?? null;
            try {
                return await stop.apply(this, args);
            } finally {
                trim._stopping = null;
            }
        };
        // The notice is caught as GNOME hands it to its source, for the moment of the call only, and gets Trim after
        // GNOME's own button, before it is shown
        ui._showNotification = function (...args) {
            const path = trim._stopping;
            if (!path)
                return show.apply(this, args);
            const proto = MessageTray.Source.prototype;
            const add = proto.addNotification;
            proto.addNotification = function (notification, ...rest) {
                try {
                    trim._addTrim(path, notification);
                } catch (e) {
                    console.error(`Amethystora UI: Trim could not be added to the recording's notice: ${e.message}`);
                }
                return add.call(this, notification, ...rest);
            };
            try {
                return show.apply(this, args);
            } finally {
                proto.addNotification = add;
            }
        };
    }

    _unwrap() {
        if (!this._wrapped)
            return;
        this._wrapped = false;
        const ui = this._ui;
        for (const [name, descriptor] of [['stopScreencast', this._stopDescriptor], ['_showNotification', this._showDescriptor]]) {
            if (descriptor)
                Object.defineProperty(ui, name, descriptor);
            else
                delete ui[name];
        }
        this._stopDescriptor = this._showDescriptor = null;
        this._stopping = null;
    }

    // The recording's notice, as GNOME adds it: Trim, while the recording is there
    _addTrim(path, notification) {
        if (!notification?.addAction || !Gio.File.new_for_path(path).query_exists(null))
            return;
        notification.addAction(_('Trim'), () => this.open(path));
    }

    // The trimming window on a recording, the path one argument of its own
    open(path) {
        try {
            const launcher = new Gio.SubprocessLauncher({flags: Gio.SubprocessFlags.INHERIT_FDS});
            launcher.spawnv([TRIM, '--', path]);
        } catch (e) {
            Main.notifyError(_('The recording could not be opened for trimming'), e.message);
        }
    }

    get wrapped() {
        return !!this._wrapped;
    }
}
