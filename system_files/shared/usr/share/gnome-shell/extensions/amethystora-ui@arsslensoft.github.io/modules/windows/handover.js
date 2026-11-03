// Hand it over (R7-64): a line in a window's menu pins that window, so that a friend can look something up or a child
// can play one game. The window is held full screen, banners wait, the other screens are covered, and the shortcuts
// that leave it go to the app instead. Ending it is the ordinary lock screen: GNOME turns the extension off as it locks,
// and that puts everything back. If the app's last window closes, the screen locks.
//
// It is a courtesy and not a boundary, and the dialog says so before anything happens: what the app itself can open is
// still reachable, and nothing here keeps out someone who means to get past it. Nothing runs and nothing is connected
// until a person chooses the line, and nothing is written.

import Clutter from 'gi://Clutter';
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import Meta from 'gi://Meta';
import Shell from 'gi://Shell';
import St from 'gi://St';

import {gettext as _} from 'resource:///org/gnome/shell/extensions/extension.js';
import * as Dialog from 'resource:///org/gnome/shell/ui/dialog.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import * as ModalDialog from 'resource:///org/gnome/shell/ui/modalDialog.js';

const LOCKDOWN = 'org.gnome.desktop.lockdown';
// The chip says its piece at full strength, then steps back until the pointer comes to it
const CHIP_QUIET_S = 5;
const CHIP_QUIET_OPACITY = 150;
// If the screen has not locked this long after the chip asked, ending stays possible: nobody is shut in
const LOCK_WAIT_S = 3;
const SUPER = Clutter.ModifierType.SUPER_MASK | Clutter.ModifierType.MOD4_MASK;
const MEDIA_KEYS = 'org.gnome.settings-daemon.plugins.media-keys';
// The keys that open nothing, by their keysyms (X11's XF86 names), so that they are the same on every layout: volume,
// mute, the microphone, playback, and the brightness of the screen and the keyboard
const QUIET_KEYS = new Set([
    0x1008ff11, 0x1008ff12, 0x1008ff13, 0x1008ffb2,
    0x1008ff14, 0x1008ff15, 0x1008ff16, 0x1008ff17, 0x1008ff31,
    0x1008ff02, 0x1008ff03, 0x1008ff05, 0x1008ff06,
]);
// The key that is L on a US layout, as the hardware numbers it: the same key whatever letter the layout puts on it
const L_KEYCODE = 46;
const ACCEL_MODS = {
    super: Clutter.ModifierType.SUPER_MASK, control: Clutter.ModifierType.CONTROL_MASK,
    primary: Clutter.ModifierType.CONTROL_MASK, ctrl: Clutter.ModifierType.CONTROL_MASK,
    alt: Clutter.ModifierType.MOD1_MASK, shift: Clutter.ModifierType.SHIFT_MASK,
};

// "<Super>l" as [modifiers, key name], or null for what is not one
function parseAccel(text) {
    const match = /^((?:<[A-Za-z]+>)*)([A-Za-z0-9_]+)$/.exec(text);
    if (!match)
        return null;
    let mask = 0;
    for (const word of match[1].matchAll(/<([A-Za-z]+)>/g))
        mask |= ACCEL_MODS[word[1].toLowerCase()] ?? 0;
    // Only a key that is a letter or a digit: any other is not told from its name here
    return match[2].length === 1 ? [mask, match[2].toLowerCase()] : null;
}

export class HandOver {
    constructor(module) {
        this._module = module;
    }

    enable() {
        this._active = false;
    }

    disable() {
        // The lock screen ends it, and any other way of stopping (the key turned off from a terminal the app opened, the
        // extension turned off) must not leave the desktop open to whoever holds the computer
        const locked = Main.sessionMode.isLocked;
        const wasActive = this._active;
        this._dialog?.close();
        this._dialog = null;
        this.stop();
        if (wasActive && !locked)
            this._lock();
    }

    get active() {
        return this._active ?? false;
    }

    get window() {
        return this._window ?? null;
    }

    // Only where a lock screen can end it, and for a window a person could use on its own
    _offered(window) {
        if (this._active || !Main.screenShield || !window.get_compositor_private() ||
            window.is_override_redirect() || window.get_window_type() !== Meta.WindowType.NORMAL ||
            !window.can_close())
            return false;
        const lockdown = new Gio.Settings({schema_id: LOCKDOWN});
        const refused = lockdown.get_boolean('disable-lock-screen');
        lockdown.run_dispose();
        return !refused;
    }

    menuItems(menu, window) {
        if (!this._offered(window))
            return null;
        // As the menu's own lines are made, so that it lines up with them
        return menu.addAction(_('Hand this over…'), () => this.ask(window));
    }

    _name(window) {
        return this._appOf(window)?.get_name() || _('This window');
    }

    _appOf(window) {
        return Shell.WindowTracker.get_default().get_window_app(window);
    }

    // The menu is gone by the time this runs
    ask(window) {
        if (this._dialog || !this._offered(window))
            return;
        const dialog = new ModalDialog.ModalDialog({styleClass: 'amethystora-windows-handover-dialog'});
        dialog.contentLayout.add_child(new Dialog.MessageDialogContent({
            title: _('Hand this over?'),
            description: _('Your friend can use %s and nothing else: no other apps, no notifications, no switching. You get it back with your password.\n\nThis is a courtesy, not a lock. For longer, or for someone you do not trust, a guest login is the answer.')
                .format(this._name(window)) + '\n\n' +
                _('To end it, press the lock key or use the bar at the top. An app that takes over the keyboard, such as a virtual machine or a remote desktop, can stop the lock key; the bar, the screen locking by itself and the power button still work.'),
        }));
        dialog.setButtons([
            {label: _('Not now'), action: () => dialog.close(), key: Clutter.KEY_Escape},
            {
                label: _('Hand it over'),
                default: true,
                action: () => {
                    dialog.close();
                    this.start(window);
                },
            },
        ]);
        dialog.connect('destroy', () => {
            if (this._dialog === dialog)
                this._dialog = null;
        });
        this._dialog = dialog;
        dialog.open();
    }

    start(window) {
        if (this._active || !window.get_compositor_private())
            return false;
        const app = this._appOf(window);
        if (!app)
            return false;
        this._active = true;
        this._window = window;
        this._app = app;
        this._wasFullscreen = window.is_fullscreen();
        this._lockKeys = this._readLockKeys();

        this._holdBanners();
        // A video or a game in full screen is scanned out by the graphics card without being painted over: the chip,
        // which is how this ends, would not be drawn
        global.compositor.disable_unredirect();
        this._unredirected = true;
        Main.overview.hide();
        this._hold(window);

        // Every shortcut, GNOME's and the settings daemon's alike, goes to the app, but for the lock and the sound and
        // brightness keys. GNOME's own filter has already had its say by the time this one is asked.
        global.window_manager.connectObject('filter-keybinding', (wm, binding) => this._refuses(binding), this);
        global.display.connectObject('notify::focus-window', () => this._queueReturn(), this);
        global.workspace_manager.connectObject('active-workspace-changed', () => this._queueReturn(), this);
        Main.overview.connectObject('showing', () => Main.overview.hide(), this);
        Main.layoutManager.connectObject('monitors-changed', () => this._buildCovers(), this);
        app.connectObject('windows-changed', () => this._windowsChanged(), this);
        this._buildChip();
        return true;
    }

    // Banners stay held until this ends: GNOME's own menus set the flag back when they close, and the property has no
    // way to be told otherwise, so it is answered for here, on the tray itself, and taken away at the end
    _holdBanners() {
        const tray = Main.messageTray;
        let proto = Object.getPrototypeOf(tray);
        let setter = null;
        while (proto && !setter) {
            setter = Object.getOwnPropertyDescriptor(proto, 'bannerBlocked')?.set ?? null;
            proto = Object.getPrototypeOf(proto);
        }
        tray.bannerBlocked = true;
        if (!setter)
            return;
        Object.defineProperty(tray, 'bannerBlocked', {configurable: true, get: () => true, set: () => {}});
        this._bannersHeld = true;
    }

    _releaseBanners() {
        const tray = Main.messageTray;
        if (this._bannersHeld)
            delete tray.bannerBlocked;
        this._bannersHeld = false;
        tray.bannerBlocked = false;
    }

    // The settings daemon's lock key, as set now: [modifiers, key name] each
    _readLockKeys() {
        const source = Gio.SettingsSchemaSource.get_default();
        if (!source?.lookup(MEDIA_KEYS, true))
            return [];
        const settings = new Gio.Settings({schema_id: MEDIA_KEYS});
        let keys = [];
        try {
            const value = settings.get_value('screensaver');
            keys = (value.is_of_type(new GLib.VariantType('as')) ? value.deepUnpack() : [value.deepUnpack()])
                .map(parseAccel).filter(Boolean);
        } finally {
            settings.run_dispose();
        }
        return keys;
    }

    // Everything back as it was. Runs at the lock, when GNOME has already covered the screen.
    stop() {
        if (!this._active)
            return;
        this._active = false;
        if (this._returnLater)
            global.compositor.get_laters().remove(this._returnLater);
        this._returnLater = 0;
        for (const id of [this._chipTimer, this._lockTimer]) {
            if (id)
                GLib.source_remove(id);
        }
        this._chipTimer = this._lockTimer = 0;
        global.window_manager.disconnectObject(this);
        global.display.disconnectObject(this);
        global.workspace_manager.disconnectObject(this);
        Main.overview.disconnectObject(this);
        Main.layoutManager.disconnectObject(this);
        this._app?.disconnectObject(this);
        this._window?.disconnectObject(this);
        this._chip?.destroy();
        this._chip = null;
        for (const cover of this._covers ?? [])
            cover.destroy();
        this._covers = [];
        this._releaseBanners();
        if (this._unredirected)
            global.compositor.enable_unredirect();
        this._unredirected = false;
        const window = this._window;
        if (window && !this._wasFullscreen && window.get_compositor_private() && window.is_fullscreen())
            window.unmake_fullscreen();
        this._window = null;
        this._app = null;
    }

    _hold(window) {
        this._window?.disconnectObject(this);
        this._window = window;
        // An app that leaves full screen (F11, Escape in a video) would show what lies behind it
        window.connectObject('notify::fullscreen', () => {
            if (!window.is_fullscreen())
                window.make_fullscreen();
        }, this);
        if (!window.is_fullscreen())
            window.make_fullscreen();
        Main.activateWindow(window);
        // The screens that do not show it are covered, whichever window of the app is the held one now
        this._buildCovers();
    }

    _refuses(binding) {
        const name = binding.get_name();
        // Super+Esc gives the shortcuts back to a window that took them, the lock key's with the rest: a way out
        if (name === 'restore-shortcuts')
            return false;
        const event = Clutter.get_current_event();
        if (!name.startsWith('external-grab') || !event)
            return true;
        const symbol = event.get_key_symbol();
        // Sound, playback and brightness open nothing, and are the same on every layout
        if (QUIET_KEYS.has(symbol))
            return false;
        // The lock key, as the settings daemon asks for it: by its letter, by its place on the keyboard (a layout of
        // another alphabet has another letter there) and as it was set in the settings
        const state = event.get_state();
        if ((state & SUPER) && (symbol === Clutter.KEY_l || symbol === Clutter.KEY_L || event.get_key_code() === L_KEYCODE))
            return false;
        const letter = String.fromCodePoint(Clutter.keysym_to_unicode(symbol) || 32).toLowerCase();
        const mask = (state & SUPER ? Clutter.ModifierType.SUPER_MASK : 0) |
            (state & (Clutter.ModifierType.CONTROL_MASK | Clutter.ModifierType.MOD1_MASK | Clutter.ModifierType.SHIFT_MASK));
        return !(this._lockKeys ?? []).some(([wanted, key]) => wanted === mask && key === letter);
    }

    // A window of the app, or a dialog one of them opened (a portal's file chooser), is fine. Anything else: back.
    _belongs(window) {
        for (let each = window; each; each = each.get_transient_for()) {
            if (this._appOf(each) === this._app)
                return true;
        }
        return false;
    }

    _queueReturn() {
        if (this._returnLater)
            return;
        this._returnLater = global.compositor.get_laters().add(Meta.LaterType.BEFORE_REDRAW, () => {
            this._returnLater = 0;
            if (!this._active)
                return false;
            const focus = global.display.focus_window;
            const here = this._window?.located_on_workspace(global.workspace_manager.get_active_workspace());
            if (!here || !focus || !this._belongs(focus))
                Main.activateWindow(this._window);
            return false;
        });
    }

    _windowsChanged() {
        if (!this._active)
            return;
        const windows = this._app.get_windows().filter(window => window.get_window_type() === Meta.WindowType.NORMAL);
        if (windows.length === 0) {
            this._handBack();
            return;
        }
        if (!windows.includes(this._window))
            this._hold(windows[0]);
        this._nameChip();
    }

    // The screen shield is there: this is only offered where it is
    _lock() {
        try {
            Main.screenShield?.lock(true);
        } catch (e) {
            console.error(`Amethystora UI: the screen did not lock: ${e.message}`);
        }
    }

    // Asked for by the chip. A screen that does not lock must not shut anyone in.
    _handBack() {
        this._lock();
        if (this._lockTimer)
            return;
        this._lockTimer = GLib.timeout_add_seconds(GLib.PRIORITY_DEFAULT, LOCK_WAIT_S, () => {
            this._lockTimer = 0;
            if (this._active && !Main.sessionMode.isLocked && !Main.screenShield?.locked) {
                console.warn('Amethystora UI: the screen did not lock, so the window was given back');
                this.stop();
            }
            return GLib.SOURCE_REMOVE;
        });
    }

    // The other screens would show the rest of the desktop
    _buildCovers() {
        for (const cover of this._covers ?? [])
            cover.destroy();
        this._covers = [];
        const held = this._window?.get_monitor() ?? Main.layoutManager.primaryIndex;
        for (const monitor of Main.layoutManager.monitors) {
            if (monitor.index === held)
                continue;
            const cover = new St.Widget({
                style_class: 'amethystora-windows-handover-cover',
                reactive: true,
                x: monitor.x,
                y: monitor.y,
                width: monitor.width,
                height: monitor.height,
            });
            Main.layoutManager.addTopChrome(cover);
            this._covers.push(cover);
        }
        this._placeChip();
    }

    _buildChip() {
        const label = new St.Label({y_align: Clutter.ActorAlign.CENTER});
        label.clutter_text.use_markup = false;
        this._chipLabel = label;
        const chip = new St.Button({
            style_class: 'amethystora-windows-handover-chip',
            child: label,
            track_hover: true,
            reactive: true,
        });
        chip.connect('clicked', () => this._handBack());
        chip.connect('notify::hover', () => {
            chip.ease({
                opacity: chip.hover || !this._chipQuiet ? 255 : CHIP_QUIET_OPACITY,
                duration: 150,
                mode: Clutter.AnimationMode.EASE_OUT_QUAD,
            });
        });
        // Over the full-screen window, which the top bar is hidden behind
        Main.layoutManager.addTopChrome(chip);
        this._chip = chip;
        chip.connect('notify::width', () => this._placeChip());
        this._nameChip();
        this._placeChip();
        this._chipQuiet = false;
        this._chipTimer = GLib.timeout_add_seconds(GLib.PRIORITY_DEFAULT, CHIP_QUIET_S, () => {
            this._chipTimer = 0;
            this._chipQuiet = true;
            if (this._chip && !this._chip.hover)
                this._chip.ease({opacity: CHIP_QUIET_OPACITY, duration: 400, mode: Clutter.AnimationMode.EASE_OUT_QUAD});
            return GLib.SOURCE_REMOVE;
        });
    }

    _nameChip() {
        if (!this._chipLabel || !this._window)
            return;
        this._chipLabel.text = _('Only %s · hand it back with your password').format(this._name(this._window));
        if (this._chip)
            this._chip.accessible_name = this._chipLabel.text;
    }

    _placeChip() {
        if (!this._chip)
            return;
        const monitor = Main.layoutManager.monitors[this._window?.get_monitor() ?? Main.layoutManager.primaryIndex] ??
            Main.layoutManager.primaryMonitor;
        this._chip.set_position(Math.round(monitor.x + (monitor.width - this._chip.width) / 2), monitor.y + 6);
    }
}
