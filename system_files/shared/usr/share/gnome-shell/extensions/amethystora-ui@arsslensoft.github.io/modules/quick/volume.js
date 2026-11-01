// Each app's own volume, under GNOME's volume slider in quick settings, while an app plays sound: a slider and a mute
// button per app, and which speakers or headphones the sound goes to.
//
// It uses the mixer GNOME's shell already has open (no second connection to the sound server) and changes only the
// volume and the muting of the account's own apps' streams. It is built when quick settings opens and taken down when
// it closes; nothing of it runs while quick settings is closed. While it is open, only an app's own stream coming or
// going changes the rows, and a row that stays is kept as it is, so a slider being dragged or focused is never made
// again under the pointer.

import Clutter from 'gi://Clutter';
import Gvc from 'gi://Gvc';
import Meta from 'gi://Meta';
import Pango from 'gi://Pango';
import Shell from 'gi://Shell';
import St from 'gi://St';

import {gettext as _} from 'resource:///org/gnome/shell/extensions/extension.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import {Slider} from 'resource:///org/gnome/shell/ui/slider.js';
import * as Volume from 'resource:///org/gnome/shell/ui/status/volume.js';

// The most rows; with more apps a line says how many more there are and opens Sound Settings
const MOST_ROWS = 6;
// The output buttons shown at most; with more outputs the last is "More…"
const MOST_SEGMENTS = 3;
// The shell itself, and the volume controls GNOME's own input slider also leaves out (they show a level, not sound)
const SKIPPED = ['org.gnome.Shell', 'org.gnome.VolumeControl', 'org.PulseAudio.pavucontrol'];

// An app's name as two letters, for one without an icon of its own
function monogram(name) {
    const letters = [...name.replace(/[^\p{L}\p{N}]/gu, '')];
    return (letters.slice(0, 2).join('') || '?').replace(/^./, c => c.toUpperCase());
}

// An app's own stream of sound: what the rows are made of
function isAppStream(stream) {
    return stream instanceof Gvc.MixerSinkInput && !stream.is_event_stream && !stream.is_virtual &&
        !SKIPPED.includes(stream.get_application_id() ?? '');
}

// An output as GNOME's own list names it: its description, and the card it is on when it says
function outputName(device) {
    return device.origin ? `${device.description} – ${device.origin}` : device.description;
}

// What tells each output's button from the others': their full names less the words at the start and the end that they
// all share (HDMI / DisplayPort 2 Output and 3 Output: 2 and 3, which a button's width would otherwise cut off), or, for
// names the same, a number after the name. The accessible name and the line under the rows keep the full name.
function outputLabels(names) {
    const lists = names.map(name => name.split(/\s+/));
    const shortest = Math.min(...lists.map(list => list.length));
    let start = 0;
    while (start < shortest && lists.every(list => list[start] === lists[0][start]))
        start++;
    let end = 0;
    while (end < shortest - start && lists.every(list => list.at(-1 - end) === lists[0].at(-1 - end)))
        end++;
    const parts = lists.map(list => list.slice(start, list.length - end).join(' '));
    return parts.every(part => part) ? parts : names.map((name, i) => `${name} ${i + 1}`);
}

export class VolumeBlock {
    constructor() {
        // By the app's key (its id, or its name for a stream without one)
        this._rows = new Map();
        this.block = new St.BoxLayout({
            style_class: 'amethystora-quick-apps',
            orientation: Clutter.Orientation.VERTICAL,
            x_expand: true,
            visible: false,
        });
        this.block._amethystoraQuickId = 'apps';

        this.block.add_child(new St.Widget({style_class: 'amethystora-quick-divider', x_expand: true}));
        // One above the other: two outputs with long names would leave the heading no room beside them
        const header = new St.BoxLayout({style_class: 'amethystora-quick-apps-header',
            orientation: Clutter.Orientation.VERTICAL, x_expand: true});
        header.add_child(new St.Label({style_class: 'amethystora-quick-heading',
            text: _('Playing now').toLocaleUpperCase(), y_align: Clutter.ActorAlign.CENTER, x_expand: true}));
        this._pickerBin = new St.Bin({x_align: Clutter.ActorAlign.START});
        header.add_child(this._pickerBin);
        this.block.add_child(header);
        this._rowBox = new St.BoxLayout({style_class: 'amethystora-quick-apps',
            orientation: Clutter.Orientation.VERTICAL, x_expand: true});
        this.block.add_child(this._rowBox);
        this._more = new St.Button({style_class: 'amethystora-quick-more', can_focus: true,
            x_align: Clutter.ActorAlign.START, visible: false});
        this._more.connect('clicked', () => {
            Main.overview.hide();
            Main.panel.closeQuickSettings();
            Shell.AppSystem.get_default().lookup_app('gnome-sound-panel.desktop')?.activate();
        });
        this.block.add_child(this._more);
        this._footer = new St.Label({style_class: 'amethystora-quick-footer', visible: false});
        this.block.add_child(this._footer);
    }

    get _output() {
        return Main.panel.statusArea.quickSettings?._volumeOutput?._output ?? null;
    }

    opened() {
        this._mixer = Volume.getMixerControl();
        this._mixer.connectObject(
            // Only an app's own stream changes the rows: event sounds, sources and outputs come and go too
            'stream-added', (mixer, id) => {
                if (isAppStream(mixer.lookup_stream_id(id)))
                    this._queue();
            },
            // A stream of a shown row, or any while some apps are only counted in "N more"
            'stream-removed', (mixer, id) => {
                if (this._more.visible || [...this._rows.values()].some(row => row.streams.some(stream => stream.id === id)))
                    this._queue();
            },
            'active-output-update', () => this._queue(),
            'output-added', () => this._queue(),
            'output-removed', () => this._queue(),
            this);
        this._update();
    }

    closed() {
        this._mixer?.disconnectObject(this);
        this._mixer = null;
        if (this._later) {
            global.compositor.get_laters().remove(this._later);
            this._later = 0;
        }
        for (const key of [...this._rows.keys()])
            this._dropRow(key);
        this._pickerBin.set_child(null);
        this._pickerKey = null;
        this.block.visible = false;
    }

    // Several changes at once (an app opening two streams) make one update, before the next frame
    _queue() {
        if (this._later)
            return;
        this._later = global.compositor.get_laters().add(Meta.LaterType.BEFORE_REDRAW, () => {
            this._later = 0;
            if (this._mixer)
                this._update();
            return false;
        });
    }

    // The apps playing now, one entry per app, sorted by name
    _apps() {
        const apps = new Map();
        const system = Shell.AppSystem.get_default();
        for (const stream of this._mixer.get_sink_inputs()) {
            if (!isAppStream(stream))
                continue;
            const appId = stream.get_application_id() ?? '';
            const app = appId ? system.lookup_app(`${appId}.desktop`) : null;
            // The mixer calls the app's own name the stream's name, and what it plays its description
            const name = app?.get_name() || stream.name || stream.description || appId;
            if (!name)
                continue;
            const key = appId ? `id:${appId}` : `name:${name}`;
            if (!apps.has(key))
                apps.set(key, {key, name, app, streams: []});
            apps.get(key).streams.push(stream);
        }
        return [...apps.values()].sort((a, b) => a.name.localeCompare(b.name) || a.key.localeCompare(b.key));
    }

    _update() {
        const apps = this._apps();
        const shown = apps.slice(0, MOST_ROWS);
        const keys = new Set(shown.map(app => app.key));
        for (const key of [...this._rows.keys()]) {
            if (!keys.has(key))
                this._dropRow(key);
        }
        shown.forEach((app, i) => {
            const row = this._rows.get(app.key);
            if (row)
                this._setStreams(row, app.streams);
            else
                this._rows.set(app.key, this._row(app));
            const box = this._rows.get(app.key).box;
            if (this._rowBox.get_child_at_index(i) !== box)
                this._rowBox.set_child_at_index(box, i);
        });
        this._more.visible = apps.length > MOST_ROWS;
        if (this._more.visible)
            this._more.label = _('%d more in Sound Settings').format(apps.length - MOST_ROWS);
        this.block.visible = apps.length > 0;
        if (!this.block.visible)
            return;
        this._updatePicker();
        const current = this._currentOutput();
        this._footer.visible = Boolean(current);
        if (current)
            this._footer.text = _('Playing through %s.').format(outputName(current));
    }

    _row(app) {
        const box = new St.BoxLayout({style_class: 'amethystora-quick-app', x_expand: true});
        let icon;
        if (app.app) {
            icon = app.app.create_icon_texture(30);
            icon.add_style_class_name('amethystora-quick-app-icon');
        } else {
            icon = new St.Bin({style_class: 'amethystora-quick-app-mark',
                child: new St.Label({text: monogram(app.name), x_align: Clutter.ActorAlign.CENTER,
                    y_align: Clutter.ActorAlign.CENTER})});
        }
        icon.y_align = Clutter.ActorAlign.CENTER;
        const named = new St.BoxLayout({style_class: 'amethystora-quick-app-named'});
        named.add_child(icon);
        box.add_child(named);

        // An app's name is text, never markup (St.Label's default)
        const label = new St.Label({style_class: 'amethystora-quick-app-name', text: app.name,
            y_align: Clutter.ActorAlign.CENTER});
        label.clutter_text.ellipsize = Pango.EllipsizeMode.END;
        named.add_child(label);

        const slider = new Slider(0);
        slider.add_style_class_name('amethystora-quick-app-slider');
        slider.accessible_name = _('Volume of %s').format(app.name);
        slider.y_align = Clutter.ActorAlign.CENTER;
        box.add_child(slider);

        const muteIcon = new St.Icon({icon_name: 'audio-volume-high-symbolic', icon_size: 16});
        const mute = new St.Button({style_class: 'amethystora-quick-mute', child: muteIcon, can_focus: true,
            y_align: Clutter.ActorAlign.CENTER});
        box.add_child(mute);

        const row = {key: app.key, name: app.name, streams: [], box, slider, mute, muteIcon, setting: false};
        const max = () => this._mixer?.get_vol_max_norm() ?? 65536;
        row.sync = () => {
            const [first] = row.streams;
            if (!first)
                return;
            row.setting = true;
            slider.value = Math.min(1, first.volume / max());
            row.setting = false;
            const muted = row.streams.every(stream => stream.is_muted);
            muteIcon.icon_name = muted ? 'audio-volume-muted-symbolic' : 'audio-volume-high-symbolic';
            mute.accessible_name = (muted ? _('Unmute %s') : _('Mute %s')).format(app.name);
            if (muted)
                mute.add_style_pseudo_class('checked');
            else
                mute.remove_style_pseudo_class('checked');
        };
        slider.connect('notify::value', () => {
            if (row.setting)
                return;
            // No amplification for an app: from silence to the sound server's full volume
            const volume = Math.round(slider.value * max());
            for (const stream of row.streams) {
                if (stream.set_volume(volume))
                    stream.push_volume();
            }
        });
        row.toggleMute = () => {
            const muted = row.streams.every(stream => stream.is_muted);
            for (const stream of row.streams)
                stream.change_is_muted(!muted);
        };
        mute.connect('clicked', () => row.toggleMute());
        this._setStreams(row, app.streams);
        this._rowBox.add_child(box);
        return row;
    }

    // A row follows the streams of its app now, and no other
    _setStreams(row, streams) {
        const same = streams.length === row.streams.length && streams.every(stream => row.streams.includes(stream));
        if (same)
            return;
        for (const stream of row.streams)
            stream.disconnectObject(row);
        row.streams = streams;
        for (const stream of streams)
            stream.connectObject('notify::volume', row.sync, 'notify::is-muted', row.sync, row);
        row.sync();
    }

    _dropRow(key) {
        const row = this._rows.get(key);
        if (!row)
            return;
        for (const stream of row.streams)
            stream.disconnectObject(row);
        row.box.destroy();
        this._rows.delete(key);
    }

    // GNOME's own outputs, as its volume slider lists them, and the one sound goes to now
    _outputs() {
        const items = this._output?._deviceItems;
        if (!(items instanceof Map) || !this._mixer)
            return null;
        const outputs = [];
        for (const id of items.keys()) {
            const device = this._mixer.lookup_output_id(id);
            const stream = device ? this._mixer.get_stream_from_device(device) : null;
            if (device && !stream?.get_name()?.startsWith('amethystora-preset-'))
                outputs.push(device);
        }
        return outputs;
    }

    _currentOutput() {
        const sink = this._mixer?.get_default_sink();
        return sink ? this._mixer.lookup_device_from_stream(sink) : null;
    }

    // The buttons are made again only when the outputs or the one in use changed
    _updatePicker() {
        const outputs = this._outputs() ?? [];
        const currentId = this._currentOutput()?.get_id();
        const pickerKey = JSON.stringify([outputs.map(device => [device.get_id(), outputName(device)]), currentId]);
        if (pickerKey === this._pickerKey)
            return;
        this._pickerKey = pickerKey;
        this._pickerBin.set_child(null);
        if (outputs.length < 2)
            return;
        let shown = outputs;
        if (outputs.length > MOST_SEGMENTS) {
            shown = outputs.slice(0, MOST_SEGMENTS - 1);
            const active = outputs.find(device => device.get_id() === currentId);
            if (active && !shown.includes(active))
                shown[shown.length - 1] = active;
        }
        const box = new St.BoxLayout({style_class: 'amethystora-quick-outputs', y_align: Clutter.ActorAlign.CENTER});
        const labels = outputLabels(shown.map(outputName));
        for (const [i, device] of shown.entries()) {
            const button = new St.Button({style_class: 'amethystora-quick-output', label: labels[i],
                accessible_name: outputName(device), can_focus: true});
            button.get_child()?.clutter_text?.set({ellipsize: Pango.EllipsizeMode.END});
            if (device.get_id() === currentId)
                button.add_style_pseudo_class('checked');
            const id = device.get_id();
            button.connect('clicked', () => this._pick(id));
            box.add_child(button);
        }
        if (outputs.length > MOST_SEGMENTS) {
            const more = new St.Button({style_class: 'amethystora-quick-output', label: _('More…'), can_focus: true});
            more.connect('clicked', () => this._output?.menu?.open());
            box.add_child(more);
        }
        this._pickerBin.set_child(box);
    }

    // As GNOME's own output menu does
    _pick(id) {
        const device = this._mixer?.lookup_output_id(id);
        if (device)
            this._mixer.change_output(device);
    }

    // --- For the tests: rows and outputs by the names they show ---------------------------------------------------

    _rowNamed(name) {
        return [...this._rows.values()].find(row => row.name === name) ?? null;
    }

    rows() {
        return [...this._rows.values()].map(row => row.name);
    }

    setRowVolume(name, fraction) {
        const row = this._rowNamed(name);
        if (!row)
            return false;
        row.slider.value = Math.max(0, Math.min(1, fraction));
        return true;
    }

    mute(name) {
        const row = this._rowNamed(name);
        row?.toggleMute();
        return Boolean(row);
    }

    outputs() {
        return (this._outputs() ?? []).map(outputName);
    }

    pickOutput(name) {
        const device = (this._outputs() ?? []).find(each => outputName(each) === name);
        if (device)
            this._pick(device.get_id());
        return Boolean(device);
    }

    footer() {
        return this._footer.visible ? this._footer.text : '';
    }

    destroy() {
        this.closed();
        this.block.destroy();
    }
}
