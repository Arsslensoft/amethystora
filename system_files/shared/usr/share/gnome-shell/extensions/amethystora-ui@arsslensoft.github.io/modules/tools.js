// Tools at hand (plans/ui-ideas/spec-tools.md): small things people otherwise install an app or open a terminal for,
// each a key or a click away and none running until asked.
//
//   - Pick a colour from the screen, Super+Alt+C (tools/picker.js); the last eight in the search as colour:
//   - Mute the microphone in every app, Super+Alt+M, with a mark in the bar while muted (tools/microphone.js)
//   - What is using this? for Files, GNOME's busy dialog and the dock's eject (tools/users.js)
//   - QR codes, for Amethystora Clipboard's text actions (showQrCode) and Share this Wi-Fi (tools/qr.js, tools/wifi.js)
//   - Convert in Files' Scripts menu, offered and off: the key convert-in-files has /usr/libexec/amethystora-convert
//     write or take away the menu's entries
//   - Earlier versions in Files' Scripts menu: the key earlier-versions, the same way (amethystora-back-in-time)
//   - Anything opens, the card for a file nothing here opens (tools/opener.js): the key open-anything hides or shows the
//     launcher that asks for it, for this account
//   - Never out of room, in place of GNOME's low-space notice (tools/room.js): the key offer-room
//   - Is it me or the internet? in the network menus: where the connection breaks (tools/reach.js)
//
// Keys of org.gnome.shell.extensions.amethystora-ui.tools, read live: colour-picker and mute-microphone (keybindings; an
// empty list leaves one unbound), convert-in-files, earlier-versions, network-reach, open-anything, offer-room, and uninstall-takes-data,
// which modules/bazaar.js reads.

import GLib from 'gi://GLib';
import Meta from 'gi://Meta';
import Shell from 'gi://Shell';
import St from 'gi://St';

import * as Main from 'resource:///org/gnome/shell/ui/main.js';

import {ColourPicker, pickedColours, remember} from './tools/picker.js';
import {Microphone} from './tools/microphone.js';
import {Users} from './tools/users.js';
import {QrDialog, QR_LONGEST} from './tools/qr.js';
import {Reach} from './tools/reach.js';
import {WifiShare} from './tools/wifi.js';
import {Opener} from './tools/opener.js';
import {Room} from './tools/room.js';
import {keyLabel} from './quit/list.js';
import {run} from './tools/run.js';

const SCHEMA = 'org.gnome.shell.extensions.amethystora-ui.tools';
const CONVERT = '/usr/libexec/amethystora-convert';
const BACK_IN_TIME = '/usr/libexec/amethystora-back-in-time';
const OPEN_ANYTHING = '/usr/libexec/amethystora-open-anything';
// Written by the helper while Convert is in Files' menu, so that a start of the module tells cheaply whether the menu
// matches the key
const CONVERT_STATE = GLib.build_filenamev([GLib.get_user_state_dir(), 'amethystora', 'files-convert.json']);
// The same for the two others, each a file only while off, so that the ordinary case starts nothing
const VERSIONS_OFF = GLib.build_filenamev([GLib.get_user_state_dir(), 'amethystora', 'files-earlier-versions-off']);
const LAUNCHER_HIDDEN = GLib.build_filenamev([GLib.get_user_data_dir(), 'applications', 'zz-amethystora-open-anything.desktop']);

export class Tools {
    constructor(ext) {
        this._ext = ext;
    }

    // Whether Convert in Files' menu is on: the pill's drop target asks
    get convertOn() {
        return this._settings?.get_boolean('convert-in-files') ?? false;
    }

    enable() {
        this._settings = this._ext.getSettings(SCHEMA);
        this._bound = new Set();
        this._settings.connectObject(
            'changed::colour-picker', () => this._bind('colour-picker', () => this.pickColour()),
            'changed::mute-microphone', () => this._bind('mute-microphone', () => this.toggleMicrophone()),
            'changed::convert-in-files', () => this._convertMenu(),
            'changed::earlier-versions', () => this._helperSwitch(BACK_IN_TIME, 'menu', 'earlier-versions'),
            'changed::open-anything', () => this._helperSwitch(OPEN_ANYTHING, 'switch', 'open-anything'),
            'changed::network-reach', () => this._syncReach(),
            this);
        this._bind('colour-picker', () => this.pickColour());
        this._bind('mute-microphone', () => this.toggleMicrophone());

        this._microphone = new Microphone(() => this._label('mute-microphone'));
        this._microphone.enable();
        this._users = new Users();
        this._users.enable();
        this._wifi = new WifiShare();
        this._wifi.enable();
        this._opener = new Opener(this._settings);
        this._opener.enable();
        this._room = new Room(this._settings);
        this._room.enable();
        this._syncReach();

        // The menu as the key says, if it is not already
        const shown = GLib.file_test(CONVERT_STATE, GLib.FileTest.EXISTS);
        if (shown !== this._settings.get_boolean('convert-in-files'))
            this._convertMenu();
        // ...and these, for a key set while this was not running: `ame setup apply`, a lock, an image's own default
        if (GLib.file_test(VERSIONS_OFF, GLib.FileTest.EXISTS) === this._settings.get_boolean('earlier-versions'))
            this._helperSwitch(BACK_IN_TIME, 'menu', 'earlier-versions');
        if (GLib.file_test(LAUNCHER_HIDDEN, GLib.FileTest.EXISTS) === this._settings.get_boolean('open-anything'))
            this._helperSwitch(OPEN_ANYTHING, 'switch', 'open-anything');
    }

    disable() {
        this._settings?.disconnectObject(this);
        for (const key of this._bound ?? [])
            Main.wm.removeKeybinding(key);
        this._bound = null;
        this._picker?.close();
        this._picker = null;
        this._qr?.close();
        this._qr = null;
        this._room?.disable();
        this._room = null;
        this._opener?.disable();
        this._opener = null;
        this._reach?.disable();
        this._reach = null;
        this._wifi?.disable();
        this._wifi = null;
        this._users?.disable();
        this._users = null;
        this._microphone?.disable();
        this._microphone = null;
        // Disposed now rather than by the collector (dock.js says why)
        this._settings?.run_dispose();
        this._settings = null;
    }

    _bind(key, action) {
        const wanted = this._settings.get_strv(key).some(accelerator => accelerator !== '');
        if (wanted === this._bound.has(key))
            return;
        if (!wanted) {
            Main.wm.removeKeybinding(key);
            this._bound.delete(key);
            return;
        }
        const id = Main.wm.addKeybinding(key, this._settings, Meta.KeyBindingFlags.IGNORE_AUTOREPEAT,
            Shell.ActionMode.NORMAL | Shell.ActionMode.OVERVIEW | Shell.ActionMode.POPUP, action);
        if (id === Meta.KeyBindingAction.NONE)
            console.warn(`Amethystora UI: the key of ${key} could not be set`);
        this._bound.add(key);
    }

    // The network menus' drawing, while its key is on; off, the menus are GNOME's own
    _syncReach() {
        const wanted = this._settings.get_boolean('network-reach');
        if (wanted && !this._reach) {
            this._reach = new Reach();
            this._reach.enable();
        } else if (!wanted && this._reach) {
            this._reach.disable();
            this._reach = null;
        }
    }

    // The key as a person reads it, "Super+Alt+M", or ''
    _label(key) {
        const accelerator = this._settings?.get_strv(key).find(a => a !== '') ?? '';
        return accelerator ? keyLabel(accelerator) : '';
    }

    // --- The colour picker ----------------------------------------------------------------------------------------

    pickColour() {
        if (this._picker?.open) {
            this._picker.close();
            return;
        }
        if (this._picker)
            return;
        this._picker = new ColourPicker(() => {
            this._picker = null;
        });
        this._picker.start().catch(e => console.warn(`Amethystora UI: the colour picker did not open: ${e.message}`));
    }

    // The search's colour: rows, newest first
    colours() {
        return pickedColours();
    }

    // A colour picked from the search: on the clipboard again, and first among the colours
    copyColour(text) {
        if (!pickedColours().includes(text))
            return false;
        remember(text);
        St.Clipboard.get_default().set_text(St.ClipboardType.CLIPBOARD, text);
        return true;
    }

    // --- The microphone -------------------------------------------------------------------------------------------

    toggleMicrophone() {
        this._microphone?.toggle();
    }

    get microphoneMuted() {
        return this._microphone?.muted ?? false;
    }

    // --- What is using this ---------------------------------------------------------------------------------------

    showUsersOfMount(mount) {
        this._users?.showForMount(mount);
    }

    // --- QR codes -------------------------------------------------------------------------------------------------

    // Whether TEXT fits in a code (Amethystora Clipboard asks before it offers the row)
    canShowQrCode(text) {
        return typeof text === 'string' && text.trim() !== '' &&
            new TextEncoder().encode(text).length <= QR_LONGEST && GLib.file_test('/usr/bin/qrencode', GLib.FileTest.IS_EXECUTABLE);
    }

    showQrCode(text) {
        if (!this.canShowQrCode(text))
            return false;
        this._qr?.close();
        const dialog = new QrDialog(text);
        dialog.connect('destroy', () => {
            if (this._qr === dialog)
                this._qr = null;
        });
        this._qr = dialog;
        dialog.open();
        return true;
    }

    // --- Convert in Files -----------------------------------------------------------------------------------------

    _convertMenu() {
        const on = this._settings.get_boolean('convert-in-files');
        if (!GLib.file_test(CONVERT, GLib.FileTest.IS_EXECUTABLE))
            return;
        run([CONVERT, 'menu', on ? 'on' : 'off'], {timeout: 15000}).then(({ok}) => {
            if (!ok)
                console.warn('Amethystora UI: Files\' Convert menu was not written');
        }).catch(e => console.warn(`Amethystora UI: Files' Convert menu: ${e.message}`));
    }

    // Earlier versions in Files' menu and the launcher for files nothing opens, as their keys say; `ame desktop` does the same
    _helperSwitch(helper, verb, key) {
        run([helper, verb, this._settings.get_boolean(key) ? 'on' : 'off'], {timeout: 15000}).catch(e =>
            console.warn(`Amethystora UI: ${key}: ${e.message}`));
    }

    // For the suites
    state() {
        return {
            bound: [...this._bound ?? []],
            opener: this._opener?.state ?? null,
            room: this._room?.state ?? null,
            picker: this._picker?.state ?? null,
            colours: pickedColours(),
            microphone: this._microphone?.state ?? null,
            users: this._users?.state ?? null,
            qr: this._qr ? {shown: this._qr.card.shown, covered: this._qr.card.covered} : null,
            wifi: this._wifi?.state ?? null,
            reach: this._reach?.state ?? null,
        };
    }

    get picker() {
        return this._picker ?? null;
    }

    get wifi() {
        return this._wifi ?? null;
    }

    get users() {
        return this._users ?? null;
    }

    get reach() {
        return this._reach ?? null;
    }
}
