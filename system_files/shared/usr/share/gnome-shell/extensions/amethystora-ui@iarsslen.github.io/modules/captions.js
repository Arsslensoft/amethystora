// Live captions (plans/beyond/spec-r4-later.md, part B): the words of the speech this computer plays, in a dark box
// low on the screen, made on this computer. The recognizer is a program of its own, /usr/libexec/amethystora-captions,
// which listens to the monitor of the sound output and writes what it hears as lines of JSON; this module starts it
// while the key `enabled` of org.gnome.shell.extensions.amethystora-ui.captions is on and the speech model is on the
// machine (`ame desktop captions on` fetches it), and shows its words. A toggle in quick settings flips the key once
// the model is there.
//
// Nothing listens while nothing plays: the module follows the apps' playback streams through the mixer GNOME already
// has open, tells the program to play while one is not corked and to pause ten seconds after the last one stopped, and a
// paused program reads no sound and spends no time; the program itself feeds the recognizer nothing during silence.
// Off, or while the screen is locked, the program is not running, nothing of the mixer is followed and no actor exists
// but the toggle, which follows the model folder (a directory watch). The words are text, never markup, and go nowhere
// but the screen. While they show, a full-screen window is composited (not scanned out), or they would not be seen.
//
// The language (the key `language`) picks the model: English in captions/model, any other in captions/model-LANG, each
// fetched by `ame desktop captions language LANG` (plans/ui-ideas/spec-later.md, R6-32). Dictation (R5-38) uses the
// same recognizer and language, from the microphone: captions/dictation.js.
//
// For the VM suite only: state() says what runs.

import Clutter from 'gi://Clutter';
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import GObject from 'gi://GObject';
import Gvc from 'gi://Gvc';
import Pango from 'gi://Pango';
import St from 'gi://St';

import {gettext as _} from 'resource:///org/gnome/shell/extensions/extension.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import {QuickToggle, SystemIndicator} from 'resource:///org/gnome/shell/ui/quickSettings.js';
import * as Volume from 'resource:///org/gnome/shell/ui/status/volume.js';

import {Dictation} from './captions/dictation.js';

const SCHEMA = 'org.gnome.shell.extensions.amethystora-ui.captions';
// By its full path: a program of the same name earlier in the session's PATH is not started instead
const HELPER = '/usr/libexec/amethystora-captions';
// The streams that are no app's sound: the shell's own, and the volume controls that show a level
const SKIPPED = ['org.gnome.Shell', 'org.gnome.VolumeControl', 'org.PulseAudio.pavucontrol'];
// The board's timings: the box fades in over 150 ms when words come and out over 300 ms, 4 s after the last ones
const SHOW_MS = 150;
const HIDE_MS = 300;
const LINGER_MS = 4000;
// The program pauses this long after the last app's sound went: a pause between two videos keeps it listening
const QUIET_MS = 10000;
// How long the sound server's changes settle before it is asked whether the apps' streams are paused
const CORK_SETTLE_MS = 300;
// Where and how wide (logical pixels): the box's bottom 48 px above the work area's, at most 60 % of the screen and
// 960 px wide, two lines at most; its padding is the stylesheet's 18 px on each side
const BOTTOM = 48;
const MOST_WIDTH = 960;
const WIDTH_SHARE = 0.6;
const PADDING_X = 18;
const LINES = 2;
// A line from the program longer than this is not one of its own
const MOST_LINE = 4096;
// Two lines hold far fewer words than this: the rest of a long sentence is not even measured
const MOST_WORDS = 120;
// A sound server that went away is tried again this much later, once
const RETRY_MS = 3000;
// The languages a model is kept for (the helper's own table), and the file that says a model's folder is whole: the
// older models have their mfcc.conf at the top
const LANGUAGES = ['en', 'fr', 'es', 'de', 'it', 'tr', 'pt', 'nl', 'ru', 'pl'];
const OLD_LAYOUT = ['tr', 'pt'];

function markerOf(folder, language) {
    const model = language === 'en' ? 'model' : `model-${language}`;
    return OLD_LAYOUT.includes(language)
        ? GLib.build_filenamev([folder, model, 'mfcc.conf'])
        : GLib.build_filenamev([folder, model, 'conf', 'model.conf']);
}

// An app's own sound, and not one the sound server says is corked or idle (a paused player keeps its stream open)
function isPlaying(stream) {
    return stream instanceof Gvc.MixerSinkInput && !stream.is_event_stream && !stream.is_virtual &&
        !SKIPPED.includes(stream.get_application_id() ?? '') &&
        ![Gvc.MixerStreamState.IDLE, Gvc.MixerStreamState.SUSPENDED].includes(stream.state);
}

const CaptionsToggle = GObject.registerClass({
    GTypeName: 'AmethystoraCaptionsToggle',
}, class CaptionsToggle extends QuickToggle {
    _init() {
        super._init({
            title: _('Live captions'),
            iconName: 'media-view-subtitles-symbolic',
            toggleMode: false,
        });
        // The quick module orders and puts away a tile by this name
        this._amethystoraQuickId = 'captions';
    }
});

export class Captions {
    constructor(ext) {
        this._ext = ext;
    }

    enable() {
        this._settings = this._ext.getSettings(SCHEMA);
        const folder = GLib.build_filenamev([GLib.get_user_data_dir(), 'amethystora', 'captions']);
        this._folder = folder;
        this._modelFile = Gio.File.new_for_path(markerOf(folder, this.language));
        // Another language: its model is looked for, and the recognizer started again with it
        this._settings.connectObject(
            'changed::enabled', () => this._sync(),
            'changed::language', () => {
                this._modelFile = Gio.File.new_for_path(markerOf(folder, this.language));
                this._stop();
                this._sync();
            }, this);
        // The model coming and going (fetch, remove) shows and takes away the toggle. The folder is made first: a watch
        // on a path that is missing polls, one on a folder is an inotify watch and costs nothing
        if (GLib.mkdir_with_parents(folder, 0o700) === 0) {
            this._folderMonitor = Gio.File.new_for_path(folder).monitor_directory(Gio.FileMonitorFlags.WATCH_MOVES, null);
            this._folderMonitor.connectObject('changed', () => this._sync(), this);
        }
        // Never at the lock screen, whatever session modes the extension is given one day
        Main.sessionMode.connectObject('updated', () => this._sync(), this);
        this._sync();
        this._dictation = new Dictation(this._settings, this);
        this._dictation.enable();
    }

    // The captions' language, one the helper has a model for
    get language() {
        const language = this._settings?.get_string('language') ?? 'en';
        return LANGUAGES.includes(language) ? language : 'en';
    }

    // Whether LANGUAGE's model is on this computer, for dictation
    modelHere(language) {
        return !!this._folder && LANGUAGES.includes(language) &&
            GLib.file_test(markerOf(this._folder, language), GLib.FileTest.EXISTS);
    }

    disable() {
        this._dictation?.disable();
        this._dictation = null;
        this._stop();
        this._dropToggle();
        Main.sessionMode.disconnectObject(this);
        this._folderMonitor?.disconnectObject(this);
        this._folderMonitor?.cancel();
        this._folderMonitor = null;
        this._settings?.disconnectObject(this);
        this._settings?.run_dispose();
        this._settings = null;
        this._modelFile = null;
        this._folder = null;
    }

    // The toggle while the model is there, the program while the key is on too
    _sync() {
        if (!this._settings)
            return;
        const present = this._modelFile.query_exists(null);
        const on = this._settings.get_boolean('enabled') && !Main.sessionMode.isLocked;
        if (present && !this._indicator)
            this._makeToggle();
        else if (!present)
            this._dropToggle();
        if (this._toggle)
            this._toggle.checked = this._settings.get_boolean('enabled');
        if (on && present && !this._proc)
            this._start();
        else if (!(on && present))
            this._stop();
    }

    _makeToggle() {
        this._indicator = new SystemIndicator();
        this._toggle = new CaptionsToggle();
        this._toggle.connect('clicked', () => this._settings?.set_boolean('enabled', !this._settings.get_boolean('enabled')));
        this._indicator.quickSettingsItems.push(this._toggle);
        Main.panel.statusArea.quickSettings.addExternalIndicator(this._indicator);
    }

    _dropToggle() {
        this._indicator?.quickSettingsItems.forEach(item => item.destroy());
        this._indicator?.destroy();
        this._indicator = null;
        this._toggle = null;
    }

    // --- The program --------------------------------------------------------------------------------------

    _start() {
        try {
            this._proc = Gio.Subprocess.new([HELPER, 'listen', this.language],
                Gio.SubprocessFlags.STDIN_PIPE | Gio.SubprocessFlags.STDOUT_PIPE);
        } catch (e) {
            console.warn(`Amethystora UI: live captions could not start: ${e.message}`);
            this._proc = null;
            this._failed('engine');
            return;
        }
        this._cancellable = new Gio.Cancellable();
        this._stdin = this._proc.get_stdin_pipe();
        this._stdout = new Gio.DataInputStream({base_stream: this._proc.get_stdout_pipe(), close_base_stream: true});
        this._playing = false;
        this._said = [];
        this._partial = '';
        // How it ends is read from its last line, at the end of its output (GSubprocess reaps it by itself)
        this._lastError = null;
        this._read();

        this._mixer = Volume.getMixerControl();
        this._corked = false;
        this._mixer.connectObject(
            'stream-added', () => this._streamsChanged(true),
            'stream-removed', () => this._streamsChanged(true),
            'stream-changed', () => this._streamsChanged(true),
            'state-changed', () => this._streamsChanged(), this);
        this._streamsChanged(true);
    }

    _stop() {
        if (this._quietId) {
            GLib.source_remove(this._quietId);
            this._quietId = 0;
        }
        if (this._lingerId) {
            GLib.source_remove(this._lingerId);
            this._lingerId = 0;
        }
        if (this._retryId) {
            GLib.source_remove(this._retryId);
            this._retryId = 0;
        }
        if (this._corkId) {
            GLib.source_remove(this._corkId);
            this._corkId = 0;
        }
        this._corkCancellable?.cancel();
        this._corkCancellable = null;
        this._allowScanout();
        this._mixer?.disconnectObject(this);
        this._mixer = null;
        this._cancellable?.cancel();
        this._cancellable = null;
        const proc = this._proc;
        this._proc = null;
        if (proc) {
            // Its stdin closing ends it; the signal is for one that does not read
            try {
                this._stdin?.close(null);
            } catch {}
            proc.send_signal(15);
        }
        this._stdin = null;
        this._stdout = null;
        this._playing = false;
        this._box?.destroy();
        this._box = null;
        this._label = null;
    }

    // The program ended by itself. A sound server that went away (PipeWire restarted by an update) is tried once more
    // a few seconds later; anything else turns captions off and says why
    _ended() {
        const kind = this._lastError ?? 'engine';
        this._stop();
        if (kind === 'audio' && !this._retried) {
            this._retried = true;
            this._retryId = GLib.timeout_add(GLib.PRIORITY_DEFAULT, RETRY_MS, () => {
                this._retryId = 0;
                this._sync();
                return GLib.SOURCE_REMOVE;
            });
            return;
        }
        this._failed(kind);
    }

    _failed(kind) {
        this._stop();
        this._settings?.set_boolean('enabled', false);
        Main.notify(_('Live captions stopped'), kind === 'model'
            ? _('The speech model is missing or damaged. Turn captions on again: ame desktop captions on.')
            : kind === 'audio'
                ? _('No sound could be taken from the output.')
                : _('The speech recognizer stopped.'));
    }

    _write(word) {
        this._stdin?.write_bytes_async(new GLib.Bytes(new TextEncoder().encode(`${word}\n`)), GLib.PRIORITY_DEFAULT,
            this._cancellable, (stream, result) => {
                try {
                    stream.write_bytes_finish(result);
                } catch {}
            });
    }

    _read() {
        const stdout = this._stdout;
        stdout.read_line_async(GLib.PRIORITY_DEFAULT, this._cancellable, (stream, result) => {
            let line;
            try {
                [line] = stream.read_line_finish_utf8(result);
            } catch {
                line = null;
            }
            if (this._stdout !== stdout)
                return;
            // The end of its output: it ended, having said why or not
            if (line === null) {
                this._ended();
                return;
            }
            if (line.length <= MOST_LINE)
                this._heard(line);
            if (this._stdout === stdout)
                this._read();
        });
    }

    _heard(line) {
        let record;
        try {
            record = JSON.parse(line);
        } catch {
            return;
        }
        if (typeof record?.error === 'string') {
            this._lastError = record.error;
        } else if (typeof record?.text === 'string') {
            this._retried = false;
            this._said = [...this._said, record.text].slice(-LINES);
            this._partial = '';
            this._show();
        } else if (typeof record?.partial === 'string') {
            this._partial = record.partial;
            this._show();
        }
    }

    // Listening while an app plays; pausing a while after the last one stopped. Under PipeWire the mixer says nothing
    // of a paused player (its stream stays, in no state), so the sound server is asked, once its changes settle,
    // whether every app's stream is paused (corked): only when a stream comes, changes or goes, never on a timer
    _streamsChanged(changed = false) {
        if (!this._mixer)
            return;
        const open = this._mixer.get_state() === Gvc.MixerControlState.READY &&
            this._mixer.get_sink_inputs().some(isPlaying);
        if (!open)
            this._corked = false;
        else if (changed)
            this._askCorked();
        const playing = open && !this._corked;
        if (playing) {
            if (this._quietId) {
                GLib.source_remove(this._quietId);
                this._quietId = 0;
            }
            if (!this._playing) {
                this._playing = true;
                this._write('play');
            }
        } else if (this._playing && !this._quietId) {
            this._quietId = GLib.timeout_add(GLib.PRIORITY_DEFAULT, QUIET_MS, () => {
                this._quietId = 0;
                this._playing = false;
                this._write('pause');
                return GLib.SOURCE_REMOVE;
            });
        }
    }

    _askCorked() {
        if (this._corkId)
            GLib.source_remove(this._corkId);
        this._corkId = GLib.timeout_add(GLib.PRIORITY_DEFAULT, CORK_SETTLE_MS, () => {
            this._corkId = 0;
            this._corkCancellable?.cancel();
            const cancellable = new Gio.Cancellable();
            this._corkCancellable = cancellable;
            let proc;
            try {
                proc = Gio.Subprocess.new(['pactl', '--format=json', 'list', 'sink-inputs'],
                    Gio.SubprocessFlags.STDOUT_PIPE | Gio.SubprocessFlags.STDERR_SILENCE);
            } catch {
                return GLib.SOURCE_REMOVE;
            }
            proc.communicate_utf8_async(null, cancellable, (p, result) => {
                if (cancellable.is_cancelled() || this._corkCancellable !== cancellable)
                    return;
                this._corkCancellable = null;
                let inputs;
                try {
                    inputs = JSON.parse(p.communicate_utf8_finish(result)[1]);
                } catch {
                    return;
                }
                // The apps' own streams, as isPlaying() counts them: no event sound, nothing of the shell's
                const apps = (Array.isArray(inputs) ? inputs : []).filter(input => {
                    const props = input.properties ?? {};
                    return props['media.role'] !== 'event' && !props['event.id'] &&
                        !SKIPPED.includes(props['application.id'] ?? '');
                });
                const corked = apps.length > 0 && apps.every(input => input.corked === true);
                if (corked !== this._corked) {
                    this._corked = corked;
                    this._streamsChanged();
                }
            });
            return GLib.SOURCE_REMOVE;
        });
    }

    // --- The box ------------------------------------------------------------------------------------------

    _ensureBox() {
        if (this._box)
            return;
        this._label = new St.Label({style_class: 'amethystora-captions-text'});
        const text = this._label.clutter_text;
        // What a stream says is text, never markup
        text.use_markup = false;
        text.line_wrap = true;
        text.line_wrap_mode = Pango.WrapMode.WORD_CHAR;
        text.ellipsize = Pango.EllipsizeMode.NONE;
        text.line_alignment = Pango.Alignment.CENTER;
        this._box = new St.Bin({
            style_class: 'amethystora-captions-box',
            child: this._label,
            reactive: false,
            opacity: 0,
            visible: false,
        });
        // Above every window, a full-screen video's too (trackFullscreen stays false): that is what captions are for.
        // Not reactive, so clicks go through it to the window under it
        Main.layoutManager.addTopChrome(this._box);
        // Below the on-screen keyboard (and the screenshot tool, which comes above it), whose keys stay readable
        Main.uiGroup.set_child_below_sibling(this._box, Main.layoutManager.keyboardBox);
    }

    // A full-screen window is put on the screen past the compositor (direct scanout) unless the compositor is told
    // something is drawn over it, as GNOME's banners and OSDs tell it: held only while the words show
    _holdScanout() {
        if (!this._unredirectOff) {
            global.compositor.disable_unredirect();
            this._unredirectOff = true;
        }
    }

    _allowScanout() {
        if (this._unredirectOff) {
            global.compositor.enable_unredirect();
            this._unredirectOff = false;
        }
    }

    _show() {
        this._ensureBox();
        const scale = St.ThemeContext.get_for_stage(global.stage).scale_factor;
        if (!this._box.visible) {
            // The screen of the window in front, else the primary one, chosen as the box appears
            this._monitor = global.display.focus_window?.get_monitor() ?? Main.layoutManager.primaryIndex;
        }
        const area = Main.layoutManager.getWorkAreaForMonitor(this._monitor) ??
            Main.layoutManager.getWorkAreaForMonitor(Main.layoutManager.primaryIndex);
        const most = Math.min(MOST_WIDTH * scale, area.width * WIDTH_SHARE) - 2 * PADDING_X * scale;
        // The stylesheet's font, before anything is measured with it
        this._box.ensure_style();
        this._label.text = this._fit([...this._said, this._partial].filter(Boolean).join(' '), most);
        // The words' own width on one line (the label's is the width it was given last time)
        const [, natural] = this._label.clutter_text.get_preferred_width(-1);
        this._label.width = Math.ceil(Math.min(natural, most));
        const [, width] = this._box.get_preferred_width(-1);
        const [, height] = this._box.get_preferred_height(width);
        // Moved by its translation, not a fixed position: a box with a fixed position that is shown and destroyed
        // before the next frame (the extension stopping as words came) is allocated by the stage after it is gone
        this._box.translation_x = Math.round(area.x + (area.width - width) / 2);
        this._box.translation_y = Math.round(area.y + area.height - BOTTOM * scale - height);

        this._holdScanout();
        if (!this._box.visible) {
            this._box.show();
            this._box.remove_all_transitions();
            this._box.ease({opacity: 255, duration: SHOW_MS, mode: Clutter.AnimationMode.EASE_OUT_QUAD});
        } else if (this._box.opacity < 255) {
            this._box.ease({opacity: 255, duration: SHOW_MS, mode: Clutter.AnimationMode.EASE_OUT_QUAD});
        }
        if (this._lingerId)
            GLib.source_remove(this._lingerId);
        this._lingerId = GLib.timeout_add(GLib.PRIORITY_DEFAULT, LINGER_MS, () => {
            this._lingerId = 0;
            this._said = [];
            this._box?.ease({
                opacity: 0,
                duration: HIDE_MS,
                mode: Clutter.AnimationMode.EASE_IN_QUAD,
                onComplete: () => {
                    this._box?.hide();
                    this._allowScanout();
                },
            });
            return GLib.SOURCE_REMOVE;
        });
    }

    // The newest words that fit in two lines of the box, the oldest dropped from the front: the first word to keep is
    // found by halving, so a long sentence costs a few layouts, not one for each word
    _fit(words, width) {
        const text = this._label.clutter_text;
        text.text = 'M';
        const [, line] = text.get_preferred_height(width);
        const parts = words.split(' ').slice(-MOST_WORDS);
        const fits = first => {
            text.text = parts.slice(first).join(' ');
            return text.get_preferred_height(width)[1] <= line * LINES + 1;
        };
        let [low, high] = [0, parts.length - 1];
        while (low < high) {
            const middle = (low + high) >> 1;
            if (fits(middle))
                high = middle;
            else
                low = middle + 1;
        }
        text.text = parts.slice(low).join(' ');
        return text.text;
    }

    // For the VM suite: what runs, and what is shown
    state() {
        return {
            enabled: this._settings?.get_boolean('enabled') ?? false,
            language: this.language,
            dictation: this._dictation?.state() ?? null,
            toggle: !!this._toggle,
            checked: this._toggle?.checked ?? false,
            helper: this._proc?.get_identifier() ?? null,
            playing: !!this._playing,
            quiet: !!this._quietId,
            mixer: !!this._mixer,
            scanoutHeld: !!this._unredirectOff,
            watching: !!this._folderMonitor,
            box: !!this._box,
            visible: !!this._box?.visible,
            text: this._label?.text ?? '',
            geometry: this._box ? [this._box.translation_x, this._box.translation_y, ...this._box.get_size()] : null,
        };
    }
}
