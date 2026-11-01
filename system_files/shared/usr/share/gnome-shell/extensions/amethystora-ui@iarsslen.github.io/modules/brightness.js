// Brightness of every screen: each external screen that speaks DDC/CI gets its own brightness slider in quick settings,
// after GNOME's brightness slider, and the brightness keys (GNOME's own brightness, while a built-in screen has it) move
// them together. GNOME's own sliders are for screens with a backlight it drives (a laptop's); an external screen's
// brightness is otherwise the buttons under its edge.
//
// Offered and off (the key external-screens of org.gnome.shell.extensions.amethystora-ui.brightness). The screens are
// reached with ddcutil, run as the account, which needs the display's i2c buses: `ame desktop screen-brightness on` gives
// them to the person at the machine (a udev rule, an administrator's choice) and turns the key on. ddcutil is asked only
// when quick settings opens (which screens there are, once after the module starts or the monitors change, and each
// one's brightness) and when a slider moves (at most one ddcutil per screen at a time, the last value winning). Nothing
// runs while quick settings is closed, and nothing at all while the key is off.
//
// For the suites: state(), and setForTest(index, fraction) as a person moving a slider.

import Clutter from 'gi://Clutter';
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import St from 'gi://St';

import {gettext as _} from 'resource:///org/gnome/shell/extensions/extension.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import {Slider} from 'resource:///org/gnome/shell/ui/slider.js';

const SCHEMA = 'org.gnome.shell.extensions.amethystora-ui.brightness';
// The VCP feature of a screen's brightness (MCCS: luminance)
const BRIGHTNESS = '10';
// How long ddcutil may take; a screen that does not answer by then is left out until quick settings opens again
const TIMEOUT_S = 10;
// A slider's moves are sent once it rests this long
const SETTLE_MS = 150;

// What `ddcutil detect --terse` says, as [{bus, connector, model}], valid displays only
export function parseDetect(text) {
    const displays = [];
    let current = null;
    for (const line of text.split('\n')) {
        if (/^Display \d+/.test(line)) {
            current = {bus: null, connector: null, model: null};
            displays.push(current);
            continue;
        }
        if (/^\S/.test(line)) {
            // Invalid display, Phantom display and anything else at the start of a line end a valid one
            current = null;
            continue;
        }
        if (!current)
            continue;
        const bus = line.match(/^\s+I2C bus:\s+\/dev\/i2c-(\d{1,3})\s*$/);
        if (bus)
            current.bus = Number(bus[1]);
        const connector = line.match(/^\s+DRM[ _]connector:\s+(?:card\d+-)?([A-Za-z0-9-]{1,32})\s*$/);
        if (connector)
            current.connector = connector[1];
        const monitor = line.match(/^\s+Monitor:\s+[^:]*:([^:]*):/);
        if (monitor)
            current.model = monitor[1].trim().slice(0, 60);
    }
    return displays.filter(display => Number.isInteger(display.bus));
}

// What `ddcutil getvcp 10 --terse` says: {value, max}, or null
export function parseVcp(text) {
    const match = text.match(/^VCP 10 C (\d+) (\d+)\s*$/m);
    if (!match)
        return null;
    const [value, max] = [Number(match[1]), Number(match[2])];
    return max > 0 && value <= max ? {value, max} : null;
}

// Runs ddcutil with these arguments; resolves to its output, or null when it failed
function ddcutil(args, cancellable) {
    const program = GLib.find_program_in_path('ddcutil');
    if (!program)
        return Promise.resolve(null);
    return new Promise(resolve => {
        let process;
        try {
            process = Gio.Subprocess.new(['timeout', `${TIMEOUT_S}`, program, ...args],
                Gio.SubprocessFlags.STDOUT_PIPE | Gio.SubprocessFlags.STDERR_SILENCE);
        } catch {
            resolve(null);
            return;
        }
        process.communicate_utf8_async(null, cancellable, (p, result) => {
            try {
                const [, stdout] = p.communicate_utf8_finish(result);
                resolve(p.get_successful() ? stdout ?? '' : null);
            } catch {
                resolve(null);
            }
        });
    });
}

export class Brightness {
    constructor(ext) {
        this._ext = ext;
    }

    enable() {
        this._settings = this._ext.getSettings(SCHEMA);
        this._settings.connectObject('changed::external-screens', () => this._sync(), this);
        this._sync();
    }

    disable() {
        this._settings?.disconnectObject(this);
        this._stop();
        // Disposed now rather than by the collector (dock.js says why)
        this._settings?.run_dispose();
        this._settings = null;
    }

    _sync() {
        const on = this._settings.get_boolean('external-screens');
        if (on && !this._block)
            this._start();
        else if (!on && this._block)
            this._stop();
    }

    _start() {
        this._quick = this._ext.modules?.get('quick') ?? null;
        if (!this._quick?.menu)
            return;
        this._cancellable = new Gio.Cancellable();
        this._screens = [];
        this._detected = false;
        this._block = new St.BoxLayout({style_class: 'amethystora-brightness', orientation: Clutter.Orientation.VERTICAL,
            x_expand: true, visible: false});
        this._block._amethystoraQuickId = 'screens';
        this._quick.setScreensBlock?.(this._block);
        this._quick.menu.connectObject('open-state-changed', (menu, open) => {
            if (open)
                this._opened();
        }, this);
        global.backend.get_monitor_manager().connectObject('monitors-changed', () => {
            this._detected = false;
        }, this);
        this._followKeys();
        if (this._quick.menu.isOpen)
            this._opened();
    }

    _stop() {
        this._cancellable?.cancel();
        this._cancellable = null;
        this._quick?.menu?.disconnectObject(this);
        global.backend.get_monitor_manager().disconnectObject(this);
        this._unfollowKeys();
        for (const screen of this._screens ?? []) {
            if (screen.settleId)
                GLib.source_remove(screen.settleId);
        }
        this._screens = null;
        if (this._block) {
            this._quick?.setScreensBlock?.(null);
            this._block.destroy();
        }
        this._block = null;
        this._quick = null;
    }

    // --- The keys: GNOME's own brightness moves, the screens follow by as much -------------------------------------

    _followKeys() {
        const manager = Main.brightnessManager;
        if (!manager)
            return;
        manager.connectObject('changed', () => this._watchScale(), this);
        this._watchScale();
    }

    _unfollowKeys() {
        Main.brightnessManager?.disconnectObject(this);
        this._scale?.disconnectObject(this);
        this._scale = null;
    }

    _watchScale() {
        const scale = Main.brightnessManager?.globalScale ?? null;
        if (scale === this._scale)
            return;
        this._scale?.disconnectObject(this);
        this._scale = scale;
        this._last = scale?.value ?? null;
        scale?.connectObject('notify::value', () => {
            const delta = scale.value - (this._last ?? scale.value);
            this._last = scale.value;
            if (Math.abs(delta) > 0.001)
                this._moveAll(delta);
        }, this);
    }

    _moveAll(delta) {
        for (const screen of this._screens ?? []) {
            if (screen.value === null)
                continue;
            this._want(screen, Math.max(0, Math.min(1, screen.value / screen.max + delta)));
        }
    }

    // --- Quick settings ---------------------------------------------------------------------------------------------

    async _opened() {
        const cancellable = this._cancellable;
        if (!this._detected) {
            this._detected = true;
            const text = await ddcutil(['detect', '--terse'], cancellable);
            if (cancellable.is_cancelled())
                return;
            const found = text === null ? [] : parseDetect(text);
            const names = this._monitorNames();
            const same = found.length === this._screens.length &&
                found.every((d, i) => d.bus === this._screens[i].bus);
            if (!same) {
                this._screens = found.map(d => ({bus: d.bus, connector: d.connector,
                    name: names.get(d.connector) || d.model || _('Screen'), value: null, max: 100,
                    wanted: null, running: false, settleId: 0}));
                this._build();
            }
        }
        // Each screen's brightness now, as it may have been changed with its buttons
        await Promise.all(this._screens.map(async screen => {
            const vcp = parseVcp(await ddcutil(['--bus', `${screen.bus}`, 'getvcp', BRIGHTNESS, '--terse'],
                cancellable) ?? '');
            if (cancellable.is_cancelled() || !vcp || screen.running || !this._screens?.includes(screen))
                return;
            screen.value = vcp.value;
            screen.max = vcp.max;
            this._show(screen);
        }));
    }

    // The screens' names as GNOME's displays settings show them, by connector (Mutter's monitor names)
    _monitorNames() {
        const names = new Map();
        for (const logical of global.backend.get_monitor_manager().get_logical_monitors()) {
            for (const monitor of logical.get_monitors())
                names.set(monitor.get_connector(), monitor.get_display_name());
        }
        return names;
    }

    _build() {
        const block = this._block;
        block.destroy_all_children();
        for (const screen of this._screens) {
            const row = new St.BoxLayout({style_class: 'amethystora-brightness-row', x_expand: true});
            row.add_child(new St.Icon({icon_name: 'video-display-symbolic', style_class: 'amethystora-brightness-icon',
                y_align: Clutter.ActorAlign.CENTER}));
            const words = new St.BoxLayout({orientation: Clutter.Orientation.VERTICAL, x_expand: true,
                y_align: Clutter.ActorAlign.CENTER});
            // A screen's name is its maker's text (from its EDID), text and never markup
            const top = new St.BoxLayout();
            top.add_child(new St.Label({style_class: 'amethystora-brightness-name', text: screen.name, x_expand: true}));
            screen.percent = new St.Label({style_class: 'amethystora-brightness-percent', text: ''});
            top.add_child(screen.percent);
            words.add_child(top);
            screen.slider = new Slider(0);
            screen.slider.accessible_name = _('Brightness of %s').format(screen.name);
            screen.slider.connect('notify::value', () => {
                if (!screen.setting)
                    this._want(screen, screen.slider.value);
            });
            words.add_child(screen.slider);
            row.add_child(words);
            block.add_child(row);
            screen.row = row;
            row.visible = false;
        }
        block.visible = false;
    }

    _show(screen) {
        if (!screen.slider || !this._block)
            return;
        screen.setting = true;
        screen.slider.value = screen.value / screen.max;
        screen.setting = false;
        screen.percent.text = `${Math.round(100 * screen.value / screen.max)}%`;
        screen.row.visible = true;
        this._block.visible = true;
    }

    // A slider moved (or the keys): sent once it rests, one ddcutil at a time per screen, the last value winning
    _want(screen, fraction) {
        screen.wanted = Math.round(fraction * screen.max);
        screen.value = screen.wanted;
        if (screen.percent)
            screen.percent.text = `${Math.round(100 * fraction)}%`;
        if (screen.slider && Math.abs(screen.slider.value - fraction) > 0.001) {
            screen.setting = true;
            screen.slider.value = fraction;
            screen.setting = false;
        }
        if (screen.settleId)
            GLib.source_remove(screen.settleId);
        screen.settleId = GLib.timeout_add(GLib.PRIORITY_DEFAULT, SETTLE_MS, () => {
            screen.settleId = 0;
            this._send(screen);
            return GLib.SOURCE_REMOVE;
        });
    }

    async _send(screen) {
        if (screen.running || screen.wanted === null)
            return;
        const cancellable = this._cancellable;
        screen.running = true;
        while (screen.wanted !== null && !cancellable?.is_cancelled()) {
            const value = screen.wanted;
            screen.wanted = null;
            // eslint-disable-next-line no-await-in-loop
            await ddcutil(['--bus', `${screen.bus}`, '--noverify', 'setvcp', BRIGHTNESS, `${value}`], cancellable);
        }
        screen.running = false;
    }

    // --- For the suites ---------------------------------------------------------------------------------------------

    state() {
        return {
            on: this._settings?.get_boolean('external-screens') ?? false,
            started: !!this._block,
            visible: this._block?.visible ?? false,
            following: this._scale !== null && this._scale !== undefined,
            screens: (this._screens ?? []).map(({bus, connector, name, value, max, running}) =>
                ({bus, connector, name, value, max, running})),
        };
    }

    setForTest(index, fraction) {
        const screen = this._screens?.[index];
        if (!screen?.slider)
            return false;
        screen.slider.value = Math.max(0, Math.min(1, fraction));
        return true;
    }
}
