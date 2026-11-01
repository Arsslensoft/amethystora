// A QR code any phone's camera reads: for a copied link or a short note (Amethystora Clipboard's text actions) and for
// the Wi-Fi in use (wifi.js). qrencode draws the modules from the text on its standard input; they are painted here as
// a picture, one pixel a module, scaled up without smoothing. While the screen is shared or recorded the code is not
// drawn at all, and one that shows when a share starts is taken away at once: what it holds can be a password.

import Clutter from 'gi://Clutter';
import Cogl from 'gi://Cogl';
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import GObject from 'gi://GObject';
import Pango from 'gi://Pango';
import St from 'gi://St';

import {gettext as _} from 'resource:///org/gnome/shell/extensions/extension.js';
import * as ModalDialog from 'resource:///org/gnome/shell/ui/modalDialog.js';

import {Capture} from '../pill/capture.js';
import {run} from './run.js';

const QRENCODE = '/usr/bin/qrencode';
// The most a code is asked to hold: past it the modules get too small for a phone across a desk
export const QR_LONGEST = 1000;
// Light modules round the code, as the standard asks
const QUIET = 4;

// The code's modules for TEXT: {size, dark}, dark[y * size + x] true for a dark module; null when qrencode is not
// there, failed or the text is too long
export async function modulesOf(text, cancellable = null) {
    if (typeof text !== 'string' || text === '' || new TextEncoder().encode(text).length > QR_LONGEST)
        return null;
    if (!GLib.file_test(QRENCODE, GLib.FileTest.IS_EXECUTABLE))
        return null;
    // Medium error correction; no margin of its own (QUIET is drawn here); the text from stdin, whole
    const {ok, stdout} = await run([QRENCODE, '-t', 'ASCII', '-m', '0', '-l', 'M', '-8'],
        {input: text, timeout: 5000, limit: 64 * 1024, cancellable});
    if (!ok)
        return null;
    return parseAscii(stdout);
}

// qrencode's ASCII picture: two characters a module, "##" dark and "  " light, a line a row
export function parseAscii(ascii) {
    const rows = ascii.split('\n').filter(line => line.length > 0);
    const size = rows.length;
    if (size < 21 || size > 177 || rows.some(row => row.length !== size * 2))
        return null;
    const dark = new Array(size * size);
    for (let y = 0; y < size; y++) {
        for (let x = 0; x < size; x++)
            dark[y * size + x] = rows[y][x * 2] === '#';
    }
    return {size, dark};
}

// The modules as RGBA bytes with the quiet zone: dark on white, which every camera reads
function pixelsOf({size, dark}) {
    const side = size + QUIET * 2;
    const bytes = new Uint8Array(side * side * 4).fill(255);
    for (let y = 0; y < size; y++) {
        for (let x = 0; x < size; x++) {
            if (!dark[y * size + x])
                continue;
            const at = ((y + QUIET) * side + x + QUIET) * 4;
            bytes[at] = bytes[at + 1] = bytes[at + 2] = 0;
        }
    }
    return {side, bytes};
}

// The code drawn at a whole number of pixels a module, no larger than SIZE
const QrPicture = GObject.registerClass(
class QrPicture extends St.Widget {
    _init(size) {
        super._init({style_class: 'amethystora-tools-qr-picture', width: size, height: size});
        this._size = size;
        this._picture = new Clutter.Actor({
            minification_filter: Clutter.ScalingFilter.NEAREST,
            magnification_filter: Clutter.ScalingFilter.NEAREST,
        });
        this.add_child(this._picture);
    }

    show_modules(modules) {
        const {side, bytes} = pixelsOf(modules);
        const scale = St.ThemeContext.get_for_stage(global.stage).scale_factor;
        // In logical pixels, a whole number of physical pixels a module
        const step = Math.max(1, Math.floor(this._size * scale / side)) / scale;
        const drawn = step * side;
        const context = global.stage.context.get_backend().get_cogl_context();
        const content = St.ImageContent.new_with_preferred_size(side, side);
        content.set_bytes(context, GLib.Bytes.new(bytes), Cogl.PixelFormat.RGBA_8888, side, side, side * 4);
        this._picture.set_content(content);
        this._picture.set_size(drawn, drawn);
        this._picture.set_position(Math.round((this._size - drawn) / 2), Math.round((this._size - drawn) / 2));
    }
});

// A card with the code and the words under it. Shown only while nothing captures the screen; the text is drawn into
// the code and never set as markup.
export const QrCard = GObject.registerClass(
class QrCard extends St.BoxLayout {
    _init({text, caption, detail = '', size = 220}) {
        super._init({
            style_class: 'amethystora-tools-qr-card',
            orientation: Clutter.Orientation.VERTICAL,
            x_align: Clutter.ActorAlign.CENTER,
            x_expand: true,
        });
        this._text = text;
        this._cancellable = new Gio.Cancellable();
        this._frame = new St.Bin({style_class: 'amethystora-tools-qr-frame', x_align: Clutter.ActorAlign.CENTER});
        this._picture = new QrPicture(size);
        this._hidden = new St.Label({
            style_class: 'amethystora-tools-qr-hidden',
            text: _('Hidden while the screen is shared'),
            width: size,
            height: size,
            x_align: Clutter.ActorAlign.CENTER,
            y_align: Clutter.ActorAlign.CENTER,
        });
        this._hidden.clutter_text.line_wrap = true;
        this._hidden.clutter_text.y_align = Clutter.ActorAlign.CENTER;
        this._frame.set_child(this._picture);
        this.add_child(this._frame);
        const label = new St.Label({style_class: 'amethystora-tools-qr-caption', text: caption, x_align: Clutter.ActorAlign.CENTER});
        this.add_child(label);
        if (detail) {
            const line = new St.Label({style_class: 'amethystora-tools-qr-detail', text: detail, x_align: Clutter.ActorAlign.CENTER});
            line.clutter_text.ellipsize = Pango.EllipsizeMode.MIDDLE;
            line.clutter_text.use_markup = false;
            line.set_style(`max-width: ${size + 40}px;`);
            this.add_child(line);
            this._detail = line;
        }
        this._capture = new Capture();
        this._unlisten = this._capture.listen(() => this._sync());
        this.connect('destroy', () => this._teardown());
        this.ready = false;
        this.failed = false;
        this._sync();
        modulesOf(text, this._cancellable).then(modules => {
            if (!this._capture)
                return;
            if (!modules) {
                this.failed = true;
                this._hidden.text = _('The code could not be made');
                this._frame.set_child(this._hidden);
                return;
            }
            this._picture.show_modules(modules);
            this.ready = true;
            this._sync();
        }).catch(e => {
            if (!e.matches?.(Gio.IOErrorEnum, Gio.IOErrorEnum.CANCELLED))
                console.warn(`Amethystora UI: qrencode did not answer: ${e.message}`);
        });
    }

    // Whether the code is drawn now (for the suites)
    get shown() {
        return this.ready && !this.covered && this._frame.get_child() === this._picture;
    }

    get covered() {
        return !!this._capture?.active;
    }

    _sync() {
        if (!this._capture || this.failed)
            return;
        this._frame.set_child(this._capture.active ? this._hidden : this._picture);
        // The text itself goes too: it may be what the code holds
        if (this._detail)
            this._detail.visible = !this._capture.active;
        if (this._capture.active)
            this._hidden.text = _('Hidden while the screen is shared');
    }

    _teardown() {
        this._cancellable?.cancel();
        this._cancellable = null;
        this._unlisten?.();
        this._unlisten = null;
        this._capture?.destroy();
        this._capture = null;
        // The child not in the frame is not destroyed with it
        for (const actor of [this._picture, this._hidden]) {
            if (!actor.get_parent())
                actor.destroy();
        }
    }
});

// The code in a dialog of its own, over everything: Close and Esc close it
export const QrDialog = GObject.registerClass(
class QrDialog extends ModalDialog.ModalDialog {
    _init(text) {
        super._init({styleClass: 'amethystora-tools-qr-dialog', destroyOnClose: true});
        this.card = new QrCard({text, caption: _('Scan with your phone'), detail: text.replace(/\s+/g, ' ').trim()});
        this.contentLayout.add_child(this.card);
        this.setButtons([{label: _('Close'), action: () => this.close(), key: Clutter.KEY_Escape, default: true}]);
    }
});
