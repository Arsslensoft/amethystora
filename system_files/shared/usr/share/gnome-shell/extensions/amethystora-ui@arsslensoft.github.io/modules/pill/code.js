// A code from your phone (R7-23, the key codes of org.amethystora.Connect, offered and off): a one-time code in a text shows
// Copy 482913 for a minute. The shell never reads a text: the phone link's program, only while the switch is on, finds the
// code itself (apps/connect/src/codes.rs) and says the digits alone, as the signal Code, with no text, sender or phone. The
// code is in memory only, for that minute: never written, never logged, never in a notification. It is hidden while the
// screen is recorded or shared, and the extension is off while the screen is locked, so it goes then too.
//
// Copy puts it on the clipboard, and names it where the shell's two readers of the clipboard look before they take anything:
// Amethystora Clipboard's history and the phone link's sending both refuse exactly that text for as long as it is named. It
// comes back off when its minute ends, unless something else has been copied since. A password manager's mark cannot be
// used: GJS cannot write a selection source that offers two types (read_async takes a callback).

import Clutter from 'gi://Clutter';
import GLib from 'gi://GLib';
import St from 'gi://St';

import {gettext as _} from 'resource:///org/gnome/shell/extensions/extension.js';

import {buttonRow, cardButton, chip, retire} from './clock.js';

const SHOW_SECONDS = 60;
// What the program may say: the digits of a code, 4 to 8 of them, nothing else
const DIGITS = /^[0-9]{4,8}$/;
// Where the shell's readers of the clipboard find the text they must refuse (Amethystora Clipboard, connect/clipboard.js)
const SECRET = Symbol.for('amethystora.clipboard.secret');

// How many times a code has been copied, across a lock: a read of the clipboard that comes back after another copy is stale
let copies = 0;

export class Code {
    // connect: the phone link's module; capture: says whether the screen is recorded or shared
    constructor(activities, {connect, capture}) {
        this._activities = activities;
        this._connect = connect;
        this._capture = capture;
        this.id = 'code';
        this.plate = 'code';
        this.beside = false;
    }

    enable() {
        this._listener = {code: code => this._show(code)};
        this._connect.addListener(this._listener);
        this._unlistenCapture = this._capture.listen(() => {
            if (this._capture.active)
                this._end();
        });
    }

    disable() {
        this._connect?.removeListener(this._listener);
        this._listener = null;
        this._unlistenCapture?.();
        this._unlistenCapture = null;
        this._end();
        this._takeBack();
    }

    // Also for the harness: as if a text with this code came now. A later code does not take the place of one on screen
    _show(code) {
        if (this._capture.active || this._code || typeof code !== 'string' || !DIGITS.test(code))
            return;
        this._code = code;
        this.pill();
        this._activities.set('code', this);
        this._timer = GLib.timeout_add_seconds(GLib.PRIORITY_DEFAULT, SHOW_SECONDS, () => {
            this._timer = 0;
            this._end();
            return GLib.SOURCE_REMOVE;
        });
    }

    _end() {
        if (this._timer)
            GLib.source_remove(this._timer);
        this._timer = 0;
        this._code = null;
        if (this._activities.get('code') === this)
            this._activities.set('code', null);
        retire(this._content);
        this._content = null;
    }

    copy() {
        const code = this._code;
        if (!code)
            return;
        this._takeBack();
        copies++;
        globalThis[SECRET] = code;
        St.Clipboard.get_default().set_text(St.ClipboardType.CLIPBOARD, code);
        this._copied = GLib.timeout_add_seconds(GLib.PRIORITY_DEFAULT, SHOW_SECONDS, () => {
            this._copied = 0;
            this._takeBack();
            return GLib.SOURCE_REMOVE;
        });
        this._end();
    }

    // Off the clipboard again, unless something else was copied since
    _takeBack() {
        if (this._copied)
            GLib.source_remove(this._copied);
        this._copied = 0;
        const code = globalThis[SECRET];
        if (!code)
            return;
        const mine = copies;
        const clipboard = St.Clipboard.get_default();
        clipboard.get_text(St.ClipboardType.CLIPBOARD, (c, text) => {
            // Another copy since (the same code again, or a new one) is no business of this read: its own will see to it
            if (mine !== copies)
                return;
            if (text === code)
                clipboard.set_text(St.ClipboardType.CLIPBOARD, '');
            // Named until it is gone, so that neither reader takes it on its way off
            if (globalThis[SECRET] === code)
                delete globalThis[SECRET];
        });
    }

    get text() {
        return this._code ? _('Copy %s').format(this._code) : '';
    }

    get accessible() {
        return _('A one-time code from your phone. Open it to copy it.');
    }

    get row() {
        return {icon: 'edit-copy-symbolic', text: _('A code from your phone')};
    }

    pill() {
        if (!this._content) {
            this._content = new St.BoxLayout({style_class: 'amethystora-pill-content amethystora-pill-code'});
            this._content.add_child(new St.Icon({icon_name: 'edit-copy-symbolic', icon_size: 14, y_align: Clutter.ActorAlign.CENTER}));
            this._content.add_child(chip(this.text, () => this.copy()));
        }
        return this._content;
    }

    card(api) {
        const body = new St.BoxLayout({style_class: 'amethystora-pill-card-body', orientation: Clutter.Orientation.VERTICAL});
        body.add_child(new St.Label({style_class: 'amethystora-pill-card-label', text: _('A code from your phone')}));
        const words = new St.Label({
            style_class: 'amethystora-pill-card-text',
            text: _('From a text just now. It goes in 60 seconds, and is kept nowhere.'),
        });
        words.clutter_text.line_wrap = true;
        body.add_child(words);
        body.add_child(buttonRow([
            cardButton(_('Dismiss'), () => {
                this._end();
                api.close();
            }),
            cardButton(_('Copy'), () => {
                this.copy();
                api.close();
            }, {primary: true}),
        ]));
        return body;
    }

    // For the suites
    get shown() {
        return !!this._code;
    }
}
