// What the audio module's two menu parts share: GNOME's sound sliders, the helper they run, and a row that switches in
// place.

import Atk from 'gi://Atk';
import Clutter from 'gi://Clutter';
import Gio from 'gi://Gio';
import GObject from 'gi://GObject';
import St from 'gi://St';

import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import * as PopupMenu from 'resource:///org/gnome/shell/ui/popupMenu.js';

export const HELPER = '/usr/libexec/amethystora-sound';
// Amethystora's own presets are sinks of their own, named so: never an output to pick, nor one to give a preset to
export const PRESET_PREFIX = 'amethystora-preset-';

// GNOME's output and microphone sliders in quick settings (ui/panel.js: _volumeOutput, _volumeInput)
export function outputSlider() {
    return Main.panel.statusArea.quickSettings?._volumeOutput?._output ?? null;
}

export function inputSlider() {
    return Main.panel.statusArea.quickSettings?._volumeInput?._input ?? null;
}

// Calls attach(slider) once the slider is there: GNOME makes its sound sliders after an await at startup (ui/panel.js,
// QuickSettings._setupIndicators), which the extension can start before; then once quick settings first opens. Returns
// what stops waiting.
export function whenSlider(find, attach) {
    const slider = find();
    if (slider) {
        attach(slider);
        return () => {};
    }
    const menu = Main.panel.statusArea.quickSettings?.menu;
    const owner = {};
    menu?.connectObject('open-state-changed', () => {
        const found = find();
        if (!found)
            return;
        menu.disconnectObject(owner);
        attach(found);
    }, owner);
    return () => menu?.disconnectObject(owner);
}

// GNOME opens a sound slider's menu only when there are two devices or more to pick from (StreamSlider._sync); while a
// part has something in it, the menu opens with one too. Wraps the slider's own _sync; the return value undoes it.
export function keepMenu(slider, wanted) {
    const own = Object.hasOwn(slider, '_sync');
    const original = slider._sync;
    slider._sync = function (...args) {
        const result = original.apply(this, args);
        if (wanted())
            this.menuEnabled = true;
        return result;
    };
    slider._sync();
    return () => {
        if (own)
            slider._sync = original;
        else
            delete slider._sync;
        slider._sync();
    };
}

// Runs the helper with these arguments (an argument vector, never a shell); resolves to whether it succeeded
export function runHelper(args, cancellable = null) {
    return new Promise(resolve => {
        let process;
        try {
            process = Gio.Subprocess.new([HELPER, ...args],
                Gio.SubprocessFlags.STDOUT_SILENCE | Gio.SubprocessFlags.STDERR_PIPE);
        } catch (e) {
            console.warn(`Amethystora UI: ${HELPER} did not start: ${e.message}`);
            resolve(false);
            return;
        }
        process.communicate_utf8_async(null, cancellable, (p, result) => {
            try {
                const [, , stderr] = p.communicate_utf8_finish(result);
                if (!p.get_successful() && stderr)
                    console.warn(`Amethystora UI: ${HELPER} ${args.join(' ')}: ${stderr.trim()}`);
                resolve(p.get_successful());
            } catch {
                resolve(false);
            }
        });
    });
}

// A row with a title, a line under it and a switch, which a click (or Space, or Enter) turns in place: a menu item
// would close the menu first, and the switch and what then happened should stay in sight
export const SwitchRow = GObject.registerClass(
class AmethystoraAudioSwitchRow extends PopupMenu.PopupBaseMenuItem {
    _init(title, text, onToggle) {
        super._init({style_class: 'amethystora-audio-switch'});
        this.accessible_role = Atk.Role.CHECK_MENU_ITEM;
        const words = new St.BoxLayout({orientation: Clutter.Orientation.VERTICAL, x_expand: true,
            y_align: Clutter.ActorAlign.CENTER});
        this._title = new St.Label({style_class: 'amethystora-audio-switch-title', text: title});
        this._text = new St.Label({style_class: 'amethystora-audio-switch-text', text});
        this._text.clutter_text.line_wrap = true;
        words.add_child(this._title);
        words.add_child(this._text);
        this.add_child(words);
        this._switch = new PopupMenu.Switch(false);
        this.add_child(this._switch);
        this.label_actor = this._title;
        this._onToggle = onToggle;
        this._switch.connect('notify::state', () => {
            if (this._switch.state)
                this.add_accessible_state(Atk.StateType.CHECKED);
            else
                this.remove_accessible_state(Atk.StateType.CHECKED);
        });
    }

    get state() {
        return this._switch.state;
    }

    set state(on) {
        this._switch.state = on;
    }

    set text(text) {
        this._text.text = text;
    }

    activate() {
        this._onToggle?.();
    }
});
