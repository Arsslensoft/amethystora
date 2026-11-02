// If it's lost, it says whose it is (R7-71; off until written): the owner's one line at the bottom of the lock screen.
// GNOME runs no extension there, and this keeps it that way: the line is a plain label left in the screen shield's own
// group, which GNOME shows as it locks, so no code of ours runs behind the lock. It is kept at module scope, since a lock
// stops the extension, and goes only when the line is taken away or the extension is stopped by anything but a lock.
//
// The line is a key of its own, normally from the system's dconf (/usr/libexec/amethystora-if-found, `ame system if-found`),
// which GDM's banner shows on the login screen too. It is public by choice, as the command says before it is written. It
// is drawn as text, never markup. With no line, only the settings are connected.
//
// What is drawn is the system's value alone (the key's default, which includes the system's dconf database and never the
// account's own): a program of the account can write its own value of any key, and must not be able to put words of its
// own on the real lock screen. It is the command's checks again: 120 characters, and no control or format character.

import Clutter from 'gi://Clutter';
import St from 'gi://St';

import * as Layout from 'resource:///org/gnome/shell/ui/layout.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';

const SCHEMA = 'org.gnome.shell.extensions.amethystora-ui.found';
// In the actor itself: a lock unloads the extension's stylesheets while this stays
const STYLE = 'color: #ffffff; font-size: 16px; font-weight: 600; text-shadow: 0 1px 4px rgba(0, 0, 0, 0.8); ' +
    'padding: 6px 16px; border-radius: 999px; background-color: rgba(10, 6, 20, 0.55); margin-bottom: 56px;';

const MOST = 120;
// Control, format and line-separating characters: none is text
const NOT_TEXT = /[\p{Cc}\p{Cf}\p{Zl}\p{Zp}]/u;

let bin = null;
let label = null;

function drop() {
    bin?.destroy();
    bin = null;
    label = null;
}

export class Found {
    constructor(ext) {
        this._ext = ext;
    }

    enable() {
        this._settings = this._ext.getSettings(SCHEMA);
        this._settings.connectObject('changed::line', () => this._sync(), this);
        this._sync();
    }

    disable() {
        this._settings?.disconnectObject(this);
        // Disposed now rather than by the collector (dock.js says why)
        this._settings?.run_dispose();
        this._settings = null;
        if (!Main.sessionMode.isLocked)
            drop();
    }

    // The system's line, or nothing: never the account's own value of the key
    _line() {
        const line = this._settings.get_default_value('line').deepUnpack().trim();
        return NOT_TEXT.test(line) ? '' : [...line].slice(0, MOST).join('');
    }

    _sync() {
        const line = this._line();
        if (!line) {
            drop();
            return;
        }
        if (!bin) {
            label = new St.Label({
                style: STYLE,
                x_expand: true,
                y_expand: true,
                x_align: Clutter.ActorAlign.CENTER,
                y_align: Clutter.ActorAlign.END,
            });
            label.clutter_text.use_markup = false;
            bin = new St.Widget({layout_manager: new Clutter.BinLayout(), reactive: false});
            bin.add_constraint(new Layout.MonitorConstraint({primary: true}));
            bin.add_child(label);
            // Above GNOME's lock dialog, which it makes as it locks; the group is shown only then
            Main.layoutManager.screenShieldGroup.add_child(bin);
        }
        label.text = line;
    }

    // For the tests
    get shown() {
        return label?.text ?? null;
    }
}
