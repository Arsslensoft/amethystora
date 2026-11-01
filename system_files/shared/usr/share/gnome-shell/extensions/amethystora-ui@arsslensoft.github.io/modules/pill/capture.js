// Whether the screen is recorded or shared, and by what: GNOME's own screen recorder, and every other recording or
// sharing (a video call, a remote desktop) as Mutter hands it out. The pill's recording, the screen's dot, the glow
// and quiet all read it from here, so there is one watcher whatever is on.

import Clutter from 'gi://Clutter';
import GLib from 'gi://GLib';
import St from 'gi://St';
import Gettext from 'gettext';

import {gettext as _} from 'resource:///org/gnome/shell/extensions/extension.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';

import {buttonRow, cardButton, chip, retire} from './clock.js';
import {clock, spoken} from './text.js';

export class Capture {
    constructor() {
        this._listeners = new Set();
        this._handles = new Map();
    }

    // Starts watching while anybody listens
    listen(callback) {
        this._listeners.add(callback);
        if (this._listeners.size === 1)
            this._start();
        return () => {
            if (!this._listeners.delete(callback) || this._listeners.size > 0)
                return;
            this._stop();
        };
    }

    _start() {
        Main.screenshotUI?.connectObject('notify::screencast-in-progress', () => this._changed(), this);
        const controller = global.backend.get_remote_access_controller?.();
        controller?.connectObject('new-handle', (c, handle) => this._add(handle), this);
        this._controller = controller;
        // The controller lists nothing: what runs already is what GNOME's own indicators hold
        const seeded = [
            ...Main.panel.statusArea.quickSettings?._remoteAccess?._handles ?? [],
            ...Main.panel.statusArea.screenSharing?._handles ?? [],
        ];
        for (const handle of seeded)
            this._add(handle, true);
        this._since = this.recording ? GLib.get_monotonic_time() : 0;
    }

    _stop() {
        Main.screenshotUI?.disconnectObject(this);
        this._controller?.disconnectObject(this);
        this._controller = null;
        for (const handle of this._handles.keys())
            handle.disconnectObject(this);
        this._handles.clear();
        this._since = 0;
    }

    // For a test: a stand-in handle is given here as Mutter would
    _onNewHandle(handle) {
        this._add(handle);
    }

    _add(handle, quietly = false) {
        if (!handle || this._handles.has(handle))
            return;
        this._handles.set(handle, {recording: !!(handle.is_recording ?? handle.isRecording)});
        handle.connectObject('stopped', () => {
            handle.disconnectObject(this);
            this._handles.delete(handle);
            this._changed();
        }, this);
        if (!quietly)
            this._changed();
    }

    // GNOME's own screen recorder
    get shellRecording() {
        return Main.screenshotUI?.screencast_in_progress === true;
    }

    get recording() {
        return this.shellRecording || [...this._handles.values()].some(h => h.recording);
    }

    get sharing() {
        return [...this._handles.values()].some(h => !h.recording);
    }

    get active() {
        return this.recording || this.sharing;
    }

    // Seconds since the recording was first seen
    get recordedFor() {
        return this._since ? (GLib.get_monotonic_time() - this._since) / 1e6 : 0;
    }

    // Stops every recording: GNOME's recorder as its own Stop does, and the other handles
    stopRecording() {
        // GNOME's recorder has a handle of its own, which its Stop ends with the video saved
        if (this.shellRecording) {
            Main.screenshotUI.stopScreencast();
            return;
        }
        for (const [handle, info] of this._handles) {
            if (info.recording)
                handle.stop();
        }
    }

    stopSharing() {
        for (const [handle, info] of this._handles) {
            if (!info.recording)
                handle.stop();
        }
    }

    _changed() {
        const recording = this.recording;
        if (recording && !this._since)
            this._since = GLib.get_monotonic_time();
        else if (!recording)
            this._since = 0;
        for (const listener of [...this._listeners]) {
            try {
                listener(this);
            } catch (e) {
                console.error(`Amethystora UI: the pill's capture listener failed: ${e.message}\n${e.stack}`);
            }
        }
    }

    destroy() {
        this._listeners.clear();
        this._stop();
    }
}

// GNOME's own name for its recordings' folder, in the person's language
function screencastsFolder() {
    const videos = GLib.get_user_special_dir(GLib.UserDirectory.DIRECTORY_VIDEOS);
    const parent = videos ? GLib.path_get_basename(videos) : _('Videos');
    return `${parent} › ${Gettext.domain('gnome-shell').gettext('Screencasts')}`;
}

// The recording in the pill: a red dot that pulses for its first six beats, the time recorded and Stop
export class Recording {
    constructor(activities, notice, capture, reduceMotion) {
        this.id = 'recording';
        this.plate = '';
        this._activities = activities;
        this._notice = notice;
        this._capture = capture;
        this._reduce = reduceMotion;
    }

    enable() {
        this._unlisten = this._capture.listen(() => this._sync());
        this._sync();
    }

    disable() {
        this._unlisten?.();
        this._unlisten = null;
        this._end();
    }

    _sync() {
        if (!this._capture.recording) {
            this._end();
            return;
        }
        if (!this._tick) {
            this._tick = GLib.timeout_add_seconds(GLib.PRIORITY_DEFAULT, 1, () => {
                this._show();
                return GLib.SOURCE_CONTINUE;
            });
        }
        const first = !this._content;
        this._show();
        if (first)
            this._pulse(this._dot, 6);
    }

    _end() {
        if (this._tick)
            GLib.source_remove(this._tick);
        this._tick = 0;
        this._card = null;
        if (this._activities.get('recording') === this)
            this._activities.set('recording', null);
        retire(this._content);
        this._content = null;
        this._dot = null;
    }

    get text() {
        return `${clock(this._capture.recordedFor)} ${_('Stop')}`;
    }

    get accessible() {
        return _('Recording the screen, %s').format(spoken(this._capture.recordedFor));
    }

    get row() {
        return {icon: 'media-record-symbolic', text: _('Recording · %s').format(clock(this._capture.recordedFor))};
    }

    get pulsing() {
        return !!(this._dot?.get_transition('opacity') || this._cardDot?.get_transition('opacity'));
    }

    // The dot's beat: opacity 1, .35, 1 in 1.2 s, `beats` times, then still
    _pulse(dot, beats) {
        if (!dot || this._reduce())
            return;
        dot.remove_transition('opacity');
        dot.opacity = 255;
        dot.ease({
            opacity: 89,
            duration: 600,
            repeatCount: 2 * beats - 1,
            autoReverse: true,
            mode: Clutter.AnimationMode.EASE_IN_OUT,
            onStopped: () => (dot.opacity = 255),
        });
    }

    pill() {
        if (!this._content) {
            this._content = new St.BoxLayout({style_class: 'amethystora-pill-content amethystora-pill-recording'});
            this._dot = new St.Widget({style_class: 'amethystora-pill-rec-dot', y_align: Clutter.ActorAlign.CENTER});
            this._time = new St.Label({style_class: 'amethystora-pill-time', y_align: Clutter.ActorAlign.CENTER});
            this._content.add_child(this._dot);
            this._content.add_child(this._time);
            this._content.add_child(chip(_('Stop'), () => this.stop()));
        }
        return this._content;
    }

    _show() {
        this.pill();
        this._time.text = clock(this._capture.recordedFor);
        this._activities.set('recording', this);
        this._activities.update('recording');
        this._card?.update();
    }

    stop() {
        const shell = this._capture.shellRecording;
        this._capture.stopRecording();
        if (shell)
            this._notice(_('Saved to Screencasts'), 'folder-videos-symbolic', 3);
    }

    card(api) {
        const shell = this._capture.shellRecording;
        const body = new St.BoxLayout({style_class: 'amethystora-pill-card-body', orientation: Clutter.Orientation.VERTICAL});
        const head = new St.BoxLayout({style_class: 'amethystora-pill-recording-head'});
        this._cardDot = new St.Widget({style_class: 'amethystora-pill-rec-dot', y_align: Clutter.ActorAlign.CENTER});
        head.add_child(this._cardDot);
        head.add_child(new St.Label({style_class: 'amethystora-pill-card-label', text: _('Recording the screen'), y_align: Clutter.ActorAlign.CENTER}));
        body.add_child(head);
        const text = new St.Label({style_class: 'amethystora-pill-card-text'});
        text.clutter_text.line_wrap = true;
        body.add_child(text);
        body.add_child(buttonRow([cardButton(shell ? _('Stop and save') : _('Stop'), () => {
            this.stop();
            api.close();
        }, {primary: true})]));
        const update = () => {
            const so = clock(this._capture.recordedFor);
            text.text = this._capture.shellRecording
                ? _('%s so far. The video goes to %s when you stop.').format(so, screencastsFolder())
                : _('%s so far. The app that is recording keeps the video.').format(so);
        };
        update();
        this._card = {update};
        body.connect('destroy', () => {
            if (this._card?.update === update)
                this._card = null;
            this._cardDot = null;
        });
        return body;
    }
}
