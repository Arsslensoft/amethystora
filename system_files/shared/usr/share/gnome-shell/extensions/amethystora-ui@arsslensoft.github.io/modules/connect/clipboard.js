// The phone link's clipboard, between this computer and the phones. Under Wayland only the shell is told when the
// clipboard changes and may read it, so this is the one part of the clipboard sync that cannot be in the program.
//
// It is as small as it can be. It reads the clipboard only while the person has turned sending on, and only after a
// change; it passes on text and nothing else; and it sets the clipboard only while the person has turned receiving on,
// for a text a phone sent. Off, it does nothing at all. The two switches are the person's own, in GSettings (which
// `ame desktop phone clipboard` writes), and are never taken from the program on the bus, which anything could imitate.

import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import Meta from 'gi://Meta';
import St from 'gi://St';

import * as Main from 'resource:///org/gnome/shell/ui/main.js';

const SCHEMA = 'org.amethystora.Connect';
export const KEY_SEND = 'clipboard-send';
export const KEY_RECEIVE = 'clipboard-receive';

// The person's settings of the phone link, or null where the image does not have them. A key that is not in the
// schema would end the shell when read, so a schema that lacks one is as good as none.
export function openSettings() {
    const schema = Gio.SettingsSchemaSource.get_default()?.lookup(SCHEMA, true);
    if (!schema || !schema.has_key(KEY_SEND) || !schema.has_key(KEY_RECEIVE)) {
        console.warn(`Amethystora UI: the settings ${SCHEMA} are not installed, so no clipboard is shared with phones`);
        return null;
    }
    return new Gio.Settings({settings_schema: schema});
}

// The most that is passed on, either way, in bytes: the limit the program puts on a clipboard text itself. A copy of a
// whole document is no use on a phone, and the bus should not carry it.
const MAX_BYTES = 1024 * 1024;
// A copy is read this long after the last report of the clipboard changing. One copy can be reported more than once
// (Wayland hands a selection over in steps), and a program that copies in bursts costs one reading, of the last.
const SETTLE_MS = 120;
// Password managers mark what they copy with this type, so that clipboards do not keep it. A copy marked so is not
// even read.
const SECRET_HINT = 'x-kde-passwordManagerHint';
// How long a text set from a phone is still known as that phone's, so that the change it makes is not sent back
const ECHO_MS = 3000;

function locked() {
    return Main.sessionMode.isLocked || Main.screenShield?.locked === true;
}

// Characters are at most three bytes each in UTF-8 once surrogate pairs are counted as two, so only a text that could
// be near the limit is measured
function fits(text) {
    if (text.length * 3 <= MAX_BYTES)
        return true;
    return text.length <= MAX_BYTES && new TextEncoder().encode(text).length <= MAX_BYTES;
}

export class Bridge {
    /**
     * @param {function(string): void} push - gives the program the clipboard's new text
     */
    constructor(push) {
        this._push = push;
        this._sending = false;
        this._receiving = false;
        this._timer = 0;
        this._reads = 0;
        this._applied = null;
    }

    // Both switches, as they stand now. Sending decides whether the clipboard is watched at all.
    setSwitches(sending, receiving) {
        this._receiving = receiving;
        if (sending === this._sending)
            return;
        this._sending = sending;
        const selection = global.display.get_selection();
        if (sending) {
            selection.connectObject('owner-changed', (s, type) => this._changed(type), this);
        } else {
            selection.disconnectObject(this);
            this._cancel();
        }
    }

    stop() {
        this.setSwitches(false, false);
        this._applied = null;
    }

    // A phone's clipboard text, to be set once
    receive(text) {
        if (!this._receiving || typeof text !== 'string' || text === '' || !fits(text) || locked())
            return;
        this._applied = {text, at: GLib.get_monotonic_time() / 1000};
        St.Clipboard.get_default().set_text(St.ClipboardType.CLIPBOARD, text);
    }

    _cancel() {
        if (this._timer)
            GLib.source_remove(this._timer);
        this._timer = 0;
        // A reading already asked for is ignored when it arrives
        this._reads++;
    }

    // The primary selection, which a mere highlighting changes, is not the clipboard and is never looked at
    _changed(type) {
        if (type === Meta.SelectionType.SELECTION_CLIPBOARD)
            this._readSoon();
    }

    _readSoon() {
        if (this._timer)
            GLib.source_remove(this._timer);
        this._timer = GLib.timeout_add(GLib.PRIORITY_DEFAULT, SETTLE_MS, () => {
            this._timer = 0;
            this._read();
            return GLib.SOURCE_REMOVE;
        });
    }

    _read() {
        if (!this._sending || locked())
            return;
        const clipboard = St.Clipboard.get_default();
        if (clipboard.get_mimetypes(St.ClipboardType.CLIPBOARD).includes(SECRET_HINT))
            return;
        const read = ++this._reads;
        clipboard.get_text(St.ClipboardType.CLIPBOARD, (c, text) => {
            // Nothing for an image or files, which have no text
            if (read !== this._reads || !this._sending || locked() || !text || !fits(text))
                return;
            if (this._isEcho(text))
                return;
            this._push(text);
        });
    }

    // Whether the text is the one set from a phone moments ago
    _isEcho(text) {
        if (!this._applied)
            return false;
        if (GLib.get_monotonic_time() / 1000 - this._applied.at > ECHO_MS) {
            this._applied = null;
            return false;
        }
        return this._applied.text === text;
    }
}
