// Privacy dots: a small button in the top bar with a dot for each thing in use, the microphone (amber), the camera
// (green), the screen (red) and the location (violet), always in that order, and hidden while nothing is. A click
// lists all four, each with its own icon so that colour is never the only sign, and who uses it where the system says.
// GNOME's own four icons for the same things are hidden meanwhile, so that nothing shows twice; their quick settings
// (the microphone's slider, the location switch) stay.
//
// Everything is read from what GNOME already follows (its mixer, its camera monitor, its location agent) and from the
// capture watcher: no new connection, nothing that polls. The camera's apps are asked of PipeWire once, when the list
// opens.

import Clutter from 'gi://Clutter';
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import Gvc from 'gi://Gvc';
import Pango from 'gi://Pango';
import Shell from 'gi://Shell';
import St from 'gi://St';

import {gettext as _} from 'resource:///org/gnome/shell/extensions/extension.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import * as PanelMenu from 'resource:///org/gnome/shell/ui/panelMenu.js';
import {getMixerControl} from 'resource:///org/gnome/shell/ui/status/volume.js';
import {getGeoclueAgent} from 'resource:///org/gnome/shell/ui/status/location.js';

import {plain} from './text.js';

const ROLE = 'amethystora-privacy';
const KINDS = ['microphone', 'camera', 'screen', 'location'];
const ICONS = {
    microphone: 'audio-input-microphone-symbolic',
    camera: 'camera-web-symbolic',
    screen: 'screen-shared-symbolic',
    location: 'location-services-active-symbolic',
};
// GNOME's own icons for the same four, in its quick settings' indicators
const GNOME_ICONS = {microphone: '_volumeInput', camera: '_camera', screen: '_remoteAccess', location: '_location'};
// The shell's own indicators, which the dots stand left of; the phone's icon goes between
const RIGHT_OF_DOTS = ['amethystora-connect', 'screenRecording', 'screenSharing', 'dwellClick', 'a11y', 'keyboard', 'quickSettings'];
// Apps that show the microphone's level and so look as if they recorded (GNOME's own rule)
const LEVEL_METERS = ['org.gnome.VolumeControl', 'org.PulseAudio.pavucontrol'];
const PW_DUMP = '/usr/bin/pw-dump';
const PW_TIMEOUT = 2000;
const PW_LIMIT = 4 * 1024 * 1024;

Gio._promisify(Gio.InputStream.prototype, 'read_bytes_async');

function appName(id) {
    if (typeof id !== 'string' || id === '')
        return '';
    const app = Shell.AppSystem.get_default().lookup_app(`${id}.desktop`);
    return app?.get_name() ?? '';
}

// The apps PipeWire says use a camera now, from one run of pw-dump: its output is data, capped and parsed in a try
async function cameraApps(cancellable) {
    if (!GLib.file_test(PW_DUMP, GLib.FileTest.IS_EXECUTABLE))
        return [];
    const process = Gio.Subprocess.new([PW_DUMP], Gio.SubprocessFlags.STDOUT_PIPE | Gio.SubprocessFlags.STDERR_SILENCE);
    const timeout = GLib.timeout_add(GLib.PRIORITY_DEFAULT, PW_TIMEOUT, () => {
        process.force_exit();
        return GLib.SOURCE_REMOVE;
    });
    const chunks = [];
    let size = 0;
    // Turned off while it reads: the program goes too
    const cancelled = cancellable?.connect(() => process.force_exit()) ?? 0;
    try {
        const stream = process.get_stdout_pipe();
        for (;;) {
            // eslint-disable-next-line no-await-in-loop
            const bytes = await stream.read_bytes_async(65536, GLib.PRIORITY_DEFAULT, cancellable);
            const length = bytes.get_size();
            if (length === 0)
                break;
            size += length;
            if (size > PW_LIMIT) {
                process.force_exit();
                return [];
            }
            chunks.push(bytes.toArray());
        }
    } finally {
        GLib.source_remove(timeout);
        if (cancelled)
            cancellable.disconnect(cancelled);
    }
    const all = new Uint8Array(size);
    let at = 0;
    for (const chunk of chunks) {
        all.set(chunk, at);
        at += chunk.length;
    }
    let nodes;
    try {
        nodes = JSON.parse(new TextDecoder().decode(all));
    } catch {
        return [];
    }
    if (!Array.isArray(nodes))
        return [];
    const names = new Set();
    for (const node of nodes) {
        const info = node?.info;
        const props = info?.props;
        if (info?.state !== 'running' || props?.['media.class'] !== 'Stream/Input/Video')
            continue;
        const name = appName(props['pipewire.access.portal.app_id']) || plain(props['application.name'], 40);
        if (name)
            names.add(name);
    }
    return [...names];
}

export class Privacy {
    constructor(capture) {
        this._capture = capture;
    }

    enable() {
        this._inUse = {microphone: false, camera: false, screen: false, location: false};
        this._forced = {};
        this._hidden = new Set();
        this._cancellable = new Gio.Cancellable();

        this._mixer = getMixerControl();
        this._mixer.connectObject(
            'stream-added', () => this._readMicrophone(),
            'stream-removed', () => this._readMicrophone(),
            'stream-changed', () => this._readMicrophone(),
            'state-changed', () => this._readMicrophone(),
            'default-source-changed', () => this._readMicrophone(),
            this);
        this._agent = getGeoclueAgent();
        this._agent.connectObject('notify::in-use', () => this._readLocation(), this);
        this._unlistenCapture = this._capture.listen(() => this._readScreen());

        // GNOME makes its indicators while it starts; one not there yet is taken when it comes
        const indicators = Main.panel.statusArea.quickSettings?._indicators;
        indicators?.connectObject('child-added', () => this._takeGnomeIcons(), this);
        this._takeGnomeIcons();

        this._readMicrophone();
        this._readLocation();
        this._readScreen();
    }

    disable() {
        this._cancellable?.cancel();
        this._cancellable = null;
        this._mixer?.disconnectObject(this);
        this._mixer = null;
        this._agent?.disconnectObject(this);
        this._agent = null;
        this._camera?.disconnectObject(this);
        this._camera = null;
        this._unlistenCapture?.();
        this._unlistenCapture = null;
        Main.panel.statusArea.quickSettings?._indicators?.disconnectObject(this);
        this._giveGnomeIcons();
        this._button?.destroy();
        this._button = null;
        // Nothing that comes late (pw-dump's answer, a test's _set) reaches the rows the button took with it
        this._inUse = null;
        this._listRows = null;
        this._dots = null;
    }

    get state() {
        const state = {};
        for (const kind of KINDS)
            state[kind] = this._used(kind);
        state.visible = !!this._button?.container.visible && !!this._button?.visible;
        return state;
    }

    // A test's way in: forces one of the four on or off (null gives it back to its source)
    _set(kind, value) {
        if (value === null)
            delete this._forced[kind];
        else
            this._forced[kind] = !!value;
        this._sync();
    }

    _used(kind) {
        return this._forced?.[kind] ?? this._inUse?.[kind] ?? false;
    }

    // --- The sources ----------------------------------------------------------------------------------------------

    // In use when a source runs and something other than a level meter records from it
    _readMicrophone() {
        const mixer = this._mixer;
        if (!mixer)
            return;
        const outputs = this._recorders();
        this._inUse.microphone = outputs.length > 0 &&
            mixer.get_sources().some(source => source.state === Gvc.MixerStreamState.RUNNING);
        this._sync();
    }

    _recorders() {
        return this._mixer?.get_source_outputs().filter(output => !LEVEL_METERS.includes(output.get_application_id())) ?? [];
    }

    _readLocation() {
        this._inUse.location = !!this._agent?.in_use;
        this._sync();
    }

    _readScreen() {
        this._inUse.screen = this._capture.active;
        this._sync();
    }

    _watchCamera() {
        const monitor = Main.panel.statusArea.quickSettings?._camera?._cameraMonitor;
        if (!monitor || monitor === this._camera)
            return;
        this._camera?.disconnectObject(this);
        this._camera = monitor;
        monitor.connectObject('notify::cameras-in-use', () => {
            this._inUse.camera = !!monitor.cameras_in_use;
            this._sync();
        }, this);
        this._inUse.camera = !!monitor.cameras_in_use;
    }

    // GNOME's own icons for the four stay hidden while the dots run
    _takeGnomeIcons() {
        const quickSettings = Main.panel.statusArea.quickSettings;
        for (const name of Object.values(GNOME_ICONS)) {
            const indicator = quickSettings?.[name];
            if (!indicator || this._hidden.has(indicator))
                continue;
            this._hidden.add(indicator);
            indicator._syncIndicatorsVisible = function () {
                this.visible = false;
            };
            indicator._syncIndicatorsVisible();
        }
        this._watchCamera();
        this._sync();
    }

    _giveGnomeIcons() {
        for (const indicator of this._hidden ?? []) {
            delete indicator._syncIndicatorsVisible;
            indicator._syncIndicatorsVisible();
        }
        this._hidden?.clear();
    }

    // --- The button in the bar ------------------------------------------------------------------------------------

    _sync() {
        if (!this._inUse)
            return;
        const used = KINDS.filter(kind => this._used(kind));
        if (used.length === 0) {
            if (this._button)
                this._button.container.hide();
            this._updateList();
            return;
        }
        if (!this._button)
            this._makeButton();
        this._button.container.show();
        for (const kind of KINDS)
            this._dots[kind].visible = used.includes(kind);
        this._button.accessible_name = this._spoken(used);
        this._updateList();
    }

    _spoken(used) {
        const words = {
            microphone: [_('Microphone'), _('microphone')],
            camera: [_('Camera'), _('camera')],
            screen: [_('Screen'), _('screen')],
            location: [_('Location'), _('location')],
        };
        const names = used.map((kind, i) => words[kind][i === 0 ? 0 : 1]);
        const list = names.length === 1 ? names[0]
            : _('%s and %s').format(names.slice(0, -1).join(', '), names[names.length - 1]);
        return _('%s in use').format(list);
    }

    _makeButton() {
        Main.panel.statusArea[ROLE]?.destroy();
        const button = new PanelMenu.Button(0.5, _('Privacy'), false);
        button.add_style_class_name('amethystora-pill-privacy-button');
        const dots = new St.BoxLayout({style_class: 'amethystora-pill-dots', y_align: Clutter.ActorAlign.CENTER});
        this._dots = {};
        for (const kind of KINDS) {
            this._dots[kind] = new St.Widget({style_class: `amethystora-pill-dot amethystora-pill-dot-${kind}`, y_align: Clutter.ActorAlign.CENTER});
            dots.add_child(this._dots[kind]);
        }
        button.add_child(dots);
        button.menu.actor.add_style_class_name('amethystora-pill-pointer');
        button.menu.box.add_style_class_name('amethystora-pill-privacy-list');
        // The camera's apps are asked for each time the list opens
        button.menu.connect('open-state-changed', (menu, open) => {
            if (open)
                this._fillList(true);
        });
        // Destroyed by another hand too (a second button under the same role): its rows and dots go with it
        button.connect('destroy', () => {
            if (this._button !== button)
                return;
            this._button = null;
            this._listRows = null;
            this._dots = null;
        });
        this._button = button;
        Main.panel.addToStatusArea(ROLE, button, this._position(), 'right');
        // A menu with nothing in it does not open: the four rows are there from the start
        this._fillList(false);
    }

    _position() {
        const containers = RIGHT_OF_DOTS.map(role => Main.panel.statusArea[role]?.container)
            .filter(container => container?.get_parent() === Main.panel._rightBox);
        if (containers.length === 0)
            return 0;
        const siblings = Main.panel._rightBox.get_children();
        return Math.min(...containers.map(container => siblings.indexOf(container)));
    }

    // --- The list -------------------------------------------------------------------------------------------------

    _fillList(askCamera) {
        const box = this._button.menu.box;
        box.destroy_all_children();
        this._listRows = {};
        this._cameraNames = null;
        for (const kind of KINDS) {
            const row = new St.BoxLayout({style_class: 'amethystora-pill-privacy-row'});
            const mark = new St.Bin({style_class: `amethystora-pill-mark amethystora-pill-mark-${kind}`, y_align: Clutter.ActorAlign.CENTER});
            mark.set_child(new St.Icon({icon_name: ICONS[kind], icon_size: 16}));
            const name = new St.Label({
                style_class: 'amethystora-pill-privacy-name',
                text: {microphone: _('Microphone'), camera: _('Camera'), screen: _('Screen'), location: _('Location')}[kind],
                y_align: Clutter.ActorAlign.CENTER,
            });
            const who = new St.Label({style_class: 'amethystora-pill-privacy-who', y_align: Clutter.ActorAlign.CENTER, x_expand: true});
            who.clutter_text.ellipsize = Pango.EllipsizeMode.END;
            row.add_child(mark);
            row.add_child(name);
            row.add_child(who);
            let stop = null;
            if (kind === 'screen') {
                stop = new St.Button({style_class: 'amethystora-pill-small-button amethystora-pill-privacy-stop', label: _('Stop'), can_focus: true, y_align: Clutter.ActorAlign.CENTER});
                stop.accessible_name = _('Stop sharing the screen');
                stop.connect('clicked', () => {
                    this._capture.stopRecording();
                    this._capture.stopSharing();
                });
                row.add_child(stop);
            }
            // The microphone's mute for every app, as Super+Alt+M (the tools module; its own mute when that is off)
            let mute = null;
            if (kind === 'microphone') {
                mute = new St.Button({style_class: 'amethystora-pill-small-button amethystora-pill-privacy-mute', label: _('Mute'), can_focus: true, y_align: Clutter.ActorAlign.CENTER});
                mute.connect('clicked', () => this._toggleMute());
                row.add_child(mute);
            }
            box.add_child(row);
            this._listRows[kind] = {row, mark, who, stop, mute};
        }
        this._updateList();
        if (askCamera && this._used('camera')) {
            cameraApps(this._cancellable).then(names => {
                this._cameraNames = names;
                this._updateList();
            }).catch(e => {
                if (!e.matches?.(Gio.IOErrorEnum, Gio.IOErrorEnum.CANCELLED))
                    console.debug(`Amethystora UI: pw-dump did not answer: ${e.message}`);
            });
        }
    }

    _updateList() {
        const rows = this._listRows;
        if (!rows)
            return;
        for (const kind of KINDS) {
            const {row, who, stop} = rows[kind];
            const used = this._used(kind);
            if (used)
                row.remove_style_class_name('amethystora-pill-privacy-row-off');
            else
                row.add_style_class_name('amethystora-pill-privacy-row-off');
            who.text = used ? this._who(kind) : _('Not in use');
            if (stop)
                stop.visible = used;
            const {mute} = rows[kind];
            if (mute) {
                const muted = !!this._mixer?.get_default_source()?.is_muted;
                mute.label = muted ? _('Unmute') : _('Mute');
                mute.accessible_name = muted ? _('Turn the microphone back on') : _('Mute the microphone in every app');
                mute.visible = used || muted;
            }
        }
    }

    _toggleMute() {
        const tools = Main.extensionManager.lookup('amethystora-ui@arsslensoft.github.io')?.stateObj?.modules?.get('tools');
        if (tools) {
            tools.toggleMicrophone();
        } else {
            const source = this._mixer?.get_default_source();
            source?.change_is_muted(!source.is_muted);
        }
        this._updateList();
    }

    _who(kind) {
        switch (kind) {
        case 'microphone': {
            const names = new Set();
            for (const output of this._recorders()) {
                // The app by its launcher, else the name the program gives itself (the stream's name, its client's
                // application.name), else the stream's description (its media.name, a file or "Recording Stream")
                const name = appName(output.get_application_id()) || plain(output.get_name(), 40) ||
                    plain(output.get_description?.() ?? '', 40);
                if (name)
                    names.add(name);
            }
            const list = [...names].join(', ') || _('In use');
            return this._mixer?.get_default_source()?.is_muted ? _('%s · Muted').format(list) : list;
        }
        case 'camera':
            return this._cameraNames?.length ? this._cameraNames.join(', ') : _('In use');
        case 'screen':
            if (this._capture.shellRecording)
                return _('Screen recorder');
            return this._capture.recording ? _('Recording') : _('Shared');
        default:
            return _('In use');
        }
    }
}
