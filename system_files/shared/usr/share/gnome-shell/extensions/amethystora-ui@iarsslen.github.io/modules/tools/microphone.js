// Mute the microphone everywhere: one key (Super+Alt+M), the privacy list's microphone row or the mark's menu mutes the
// default input, the one quick settings' slider sets, for every app at once, and again turns it back on. While the
// default input is muted, whatever muted it, a mark stays in the top bar. Muted from here, a microphone chosen next
// (a headset plugged in) is muted too, until the person turns it back on.
//
// Everything is the mixer's: its default source and that source's own mute. Signals only.

import Clutter from 'gi://Clutter';
import Gio from 'gi://Gio';
import St from 'gi://St';

import {gettext as _} from 'resource:///org/gnome/shell/extensions/extension.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import * as PanelMenu from 'resource:///org/gnome/shell/ui/panelMenu.js';
import * as PopupMenu from 'resource:///org/gnome/shell/ui/popupMenu.js';
import {getMixerControl} from 'resource:///org/gnome/shell/ui/status/volume.js';

const ROLE = 'amethystora-tools-mic';
const ICON_OFF = 'microphone-disabled-symbolic';
const ICON_ON = 'audio-input-microphone-symbolic';

// Muted from here: kept at module scope, since a screen lock stops the module and the unlock starts it again
let held = false;

export class Microphone {
    constructor(keyLabel) {
        this._keyLabel = keyLabel;
    }

    enable() {
        this._mixer = getMixerControl();
        this._mixer.connectObject(
            'default-source-changed', () => this._sourceChanged(),
            'state-changed', () => this._sourceChanged(),
            this);
        this._sourceChanged();
    }

    disable() {
        this._mixer?.disconnectObject(this);
        this._mixer = null;
        this._source?.disconnectObject(this);
        this._source = null;
        this._mark?.destroy();
        this._mark = null;
    }

    get muted() {
        return !!this._source?.is_muted;
    }

    // The key, the privacy row and the mark's menu
    toggle() {
        const source = this._mixer?.get_default_source();
        if (!source) {
            Main.osdWindowManager.showAll(Gio.ThemedIcon.new(ICON_OFF), _('There is no microphone'), null);
            return;
        }
        const mute = !source.is_muted;
        held = mute;
        source.change_is_muted(mute);
        Main.osdWindowManager.showAll(Gio.ThemedIcon.new(mute ? ICON_OFF : ICON_ON),
            mute ? _('Microphone off in every app') : _('Microphone on'), null);
    }

    _sourceChanged() {
        const source = this._mixer?.get_default_source() ?? null;
        if (source !== this._source) {
            this._source?.disconnectObject(this);
            this._source = source;
            source?.connectObject('notify::is-muted', () => this._muteChanged(), this);
            // A microphone chosen while the others were muted from here starts muted
            if (source && held && !source.is_muted)
                source.change_is_muted(true);
        }
        this._sync();
    }

    _muteChanged() {
        // Turned back on from anywhere (the slider, the key): the hold goes with it
        if (this._source && !this._source.is_muted)
            held = false;
        this._sync();
    }

    _sync() {
        const muted = this.muted;
        if (muted && !this._mark)
            this._makeMark();
        if (this._mark)
            this._mark.container.visible = muted;
    }

    _makeMark() {
        Main.panel.statusArea[ROLE]?.destroy();
        const mark = new PanelMenu.Button(0.5, _('Microphone off'), false);
        mark.add_style_class_name('amethystora-tools-mic-mark');
        const box = new St.BoxLayout({style_class: 'amethystora-tools-mic-pill', y_align: Clutter.ActorAlign.CENTER});
        box.add_child(new St.Icon({icon_name: ICON_OFF, style_class: 'amethystora-tools-mic-icon'}));
        box.add_child(new St.Label({text: _('Mic off'), y_align: Clutter.ActorAlign.CENTER}));
        mark.add_child(box);
        mark.accessible_name = _('Microphone off in every app');
        const title = new PopupMenu.PopupMenuItem(_('Microphone off in every app'), {reactive: false});
        mark.menu.addMenuItem(title);
        const label = this._keyLabel();
        if (label)
            mark.menu.addMenuItem(new PopupMenu.PopupMenuItem(_('%s turns it back on').format(label), {reactive: false}));
        mark.menu.addMenuItem(new PopupMenu.PopupSeparatorMenuItem());
        mark.menu.addAction(_('Turn it back on'), () => this.toggle(), ICON_ON);
        mark.connect('destroy', () => {
            if (this._mark === mark)
                this._mark = null;
        });
        this._mark = mark;
        // Left of the privacy dots, else first among the system's icons
        const dots = Main.panel.statusArea['amethystora-privacy']?.container;
        const siblings = Main.panel._rightBox.get_children();
        const at = dots && dots.get_parent() === Main.panel._rightBox ? siblings.indexOf(dots)
            : siblings.indexOf(Main.panel.statusArea.quickSettings?.container);
        Main.panel.addToStatusArea(ROLE, mark, Math.max(0, at), 'right');
    }

    // For the suites
    get state() {
        return {source: !!this._source, muted: this.muted, held, mark: !!this._mark?.container.visible};
    }
}
