// Putting text where the person was typing, once the emoji picker or the search box has given the keyboard back: typed
// straight in through the input method, as GNOME's own on-screen keyboard types an emoji, when the window takes text
// from it; otherwise put on the clipboard and pasted with Shift+Insert, or Ctrl+Shift+V in a terminal. Insert is on
// every keyboard layout, where a V may not be (Mutter finds a key only in the layout in use). The text goes only to
// the window that had the focus when the picker or the box opened, and only if it still has it; otherwise it is left
// on the clipboard. Nothing runs while nothing is being pasted: the wait below lives a second and a half at most.

import Clutter from 'gi://Clutter';
import GLib from 'gi://GLib';
import Meta from 'gi://Meta';
import Shell from 'gi://Shell';
import St from 'gi://St';

import * as Main from 'resource:///org/gnome/shell/ui/main.js';

// How long the window has to take the keyboard back, and how often that is looked at meanwhile
const GIVE_UP = 1000;
const LOOK = 20;
// Once it has: time for the window to turn its text input on again, and how long more it may take before the text is
// pasted instead (a window without one, an X11 app, waits that long)
const SETTLE = 60;
const TEXT_INPUT = 500;
// The virtual keyboard is let go once its keys are up
const DISPOSE = 100;

const PENDING = new Set();
let rejoin = 0;

// Every wait still running stops (the module that started it stops)
export function cancelPastes() {
    for (const cancel of [...PENDING])
        cancel();
    if (rejoin)
        GLib.source_remove(rejoin);
    rejoin = 0;
}

// The window's text input back to the input method, once a field of the shell's own has had the keys: the field took
// the input method for itself, and Mutter hands it back to a Wayland window only when the window turns its text input
// on again, which it does when the keyboard enters it. The keyboard never left it (the picker and the box are not
// modal), so a grab taken and let go at once makes it leave and enter, as any menu of the shell does. Without this the
// window types without the input method (no IBus engine, no text shortcuts) until it is focused again. Not while a
// button is held (the click that closed the picker): the window would lose the press it is taking.
const BUTTONS = Clutter.ModifierType.BUTTON1_MASK | Clutter.ModifierType.BUTTON2_MASK |
    Clutter.ModifierType.BUTTON3_MASK;
export function giveBackTextInput() {
    if (rejoin)
        GLib.source_remove(rejoin);
    const started = GLib.get_monotonic_time();
    const look = () => {
        rejoin = 0;
        if (Main.modalCount > 0 || global.stage.key_focus || !global.display.focus_window ||
            Main.inputMethod.currentFocus)
            return;
        if (global.get_pointer()[2] & BUTTONS) {
            if (GLib.get_monotonic_time() - started < GIVE_UP * 1000)
                rejoin = GLib.timeout_add_once(GLib.PRIORITY_DEFAULT, LOOK, look);
            return;
        }
        global.stage.grab(Main.layoutManager.uiGroup).dismiss();
    };
    rejoin = GLib.timeout_add_once(GLib.PRIORITY_DEFAULT, 0, look);
}

function putOnClipboard(text) {
    if (text)
        St.Clipboard.get_default().set_text(St.ClipboardType.CLIPBOARD, text);
}

function isTerminal(window) {
    if (Main.inputMethod.content_purpose === Clutter.InputContentPurpose.TERMINAL)
        return true;
    const app = window ? Shell.WindowTracker.get_default().get_window_app(window) : null;
    return (app?.get_app_info()?.get_categories() ?? '').split(';').includes('TerminalEmulator');
}

function press(keys) {
    const seat = global.stage.context.get_backend().get_default_seat();
    const device = seat.create_virtual_device(Clutter.InputDeviceType.KEYBOARD_DEVICE);
    let time = GLib.get_monotonic_time();
    for (const key of keys)
        device.notify_keyval(time++, key, Clutter.KeyState.PRESSED);
    for (const key of [...keys].reverse())
        device.notify_keyval(time++, key, Clutter.KeyState.RELEASED);
    GLib.timeout_add_once(GLib.PRIORITY_DEFAULT, DISPOSE, () => device.run_dispose());
}

// {text, keysOnly, window, keep}: keysOnly when the clipboard already holds what is to be pasted (a picture, say); keep
// when the clipboard holds the text already and must never be written (a password a password manager marks secret,
// which the clipboard's history would keep once it is put there again): typed in, or pasted with the keys as it is
export function paste({text = null, keysOnly = false, window = null, keep = false}) {
    const onClipboard = keep ? () => {} : putOnClipboard;
    const started = GLib.get_monotonic_time();
    let source = 0;
    const stop = () => {
        if (source)
            GLib.source_remove(source);
        source = 0;
        PENDING.delete(stop);
    };
    PENDING.add(stop);

    let settled = 0;
    const deliver = () => {
        source = 0;
        // The person went to another window meanwhile, or the shell took the keyboard again (the overview, a menu):
        // the text waits on the clipboard
        if (!ready() || !window || !window.get_compositor_private() || global.display.focus_window !== window) {
            stop();
            onClipboard(text);
            return;
        }
        // The window has the keyboard back but has not turned its text input on again yet (Mutter gives the input
        // method its focus once the app asks for it): it is given a little longer, looked at as before
        if (text && !keysOnly && !Main.inputMethod.currentFocus && !isTerminal(window)) {
            settled ||= GLib.get_monotonic_time();
            if (GLib.get_monotonic_time() - settled < TEXT_INPUT * 1000) {
                source = GLib.timeout_add_once(GLib.PRIORITY_DEFAULT, LOOK, deliver);
                return;
            }
        }
        stop();
        if (text && !keysOnly && Main.inputMethod.currentFocus &&
            Main.inputMethod.content_purpose !== Clutter.InputContentPurpose.TERMINAL) {
            Main.inputMethod.commit(text);
            return;
        }
        onClipboard(text);
        press(isTerminal(window)
            ? [Clutter.KEY_Control_L, Clutter.KEY_Shift_L, Clutter.KEY_v]
            : [Clutter.KEY_Shift_L, Clutter.KEY_Insert]);
    };

    const ready = () => Main.modalCount === 0 && !global.stage.key_focus;
    const look = () => {
        source = 0;
        if (ready()) {
            // A frame, then the window's text input back on
            const laters = global.compositor.get_laters();
            laters.add(Meta.LaterType.BEFORE_REDRAW, () => {
                if (PENDING.has(stop))
                    source = GLib.timeout_add_once(GLib.PRIORITY_DEFAULT, SETTLE, deliver);
                return GLib.SOURCE_REMOVE;
            });
            return;
        }
        if (GLib.get_monotonic_time() - started > GIVE_UP * 1000) {
            stop();
            onClipboard(text);
            return;
        }
        source = GLib.timeout_add_once(GLib.PRIORITY_DEFAULT, LOOK, look);
    };
    look();
}
