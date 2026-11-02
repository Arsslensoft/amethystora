// Hide what's private (R7-67): one button in the screenshot bar has the picture read on this computer, and draws a solid
// box, a filled mark like the toolbar's own, over each e-mail address, card, phone and bank account number found. Solid,
// never blurred: what a blur hides can sometimes be worked out again, and the box is burnt into the picture with the
// other marks. It will miss things, so a card says what was covered and to check, with Uncover and Copy.
//
// The picture goes to the helper through a pipe and its answer holds places and kinds, never words: nothing is written
// and nothing is kept once the screenshot UI closes. The helper runs only after the click.

import Clutter from 'gi://Clutter';
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import St from 'gi://St';

import {gettext as _, ngettext} from 'resource:///org/gnome/shell/extensions/extension.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';

import {captureFrame} from './capture.js';
import {newMark} from './marks.js';

// In a scope of its own, as the mirror is: the reading of a picture must not weigh on GNOME Shell's cgroup, and runs out
// of its memory in its own
const HELPER = ['systemd-run', '--user', '--scope', '--quiet', '--collect', '-p', 'MemoryMax=400M', '-p', 'CPUWeight=50',
    '/usr/libexec/amethystora-cover-private'];
const READ_MS = 30000;
const BLACK = '#000000';
const KINDS = ['address', 'card', 'phone', 'account'];
// As wide as stylesheets/screenshot.css draws it, and below the toolbar
const CARD_WIDTH = 380;
const CARD_TOP = 72;

Gio._promisify(Gio.Subprocess.prototype, 'communicate_async');

// "an address", "2 card numbers", in the words of each kind's count
function named(kind, count) {
    switch (kind) {
    case 'address':
        return ngettext('an address', '%d addresses', count).format(count);
    case 'card':
        return ngettext('a card number', '%d card numbers', count).format(count);
    case 'phone':
        return ngettext('a phone number', '%d phone numbers', count).format(count);
    default:
        return ngettext('an account number', '%d account numbers', count).format(count);
    }
}

// The sentence is whole, and each kind found is a line of its own beneath it: no sentence is built from pieces
function sentence(counts) {
    const total = KINDS.reduce((sum, kind) => sum + (counts[kind] ?? 0), 0);
    return ngettext('%d thing covered. Check before you send.', '%d things covered. Check before you send.', total)
        .format(total);
}

function lines(counts) {
    return KINDS.filter(kind => counts[kind]).map(kind => named(kind, counts[kind]));
}

// Only what the helper can say: four whole numbers and a known kind
function checked(answer, width, height) {
    if (!Array.isArray(answer?.boxes))
        return null;
    return answer.boxes.filter(box => KINDS.includes(box?.kind) &&
        [box.x, box.y, box.w, box.h].every(value => Number.isInteger(value) && value >= 0) &&
        box.w > 0 && box.h > 0 && box.x < width && box.y < height);
}

export class Cover {
    // session gives the marks, redraws them and takes the copy; overlay is where the card lives
    constructor(session, ui, overlay) {
        this._session = session;
        this._ui = ui;
        this._overlay = overlay;
        this._marks = [];
    }

    destroy() {
        this._cancellable?.cancel();
        this._cancellable = null;
        this._card?.destroy();
        this._card = null;
        this._marks = [];
    }

    get reading() {
        return Boolean(this._cancellable);
    }

    // The button, again as often as the area changes. What it covered before stays until the new reading is in: a
    // second press never takes boxes off unasked, and a picture taken meanwhile is refused (wait)
    async run() {
        if (this._cancellable)
            return;
        const ui = this._ui;
        const scale = ui._scale;
        const [x, y] = ui._getSelectedGeometry(false);
        const cancellable = new Gio.Cancellable();
        this._cancellable = cancellable;
        this._show(_('Reading the picture, on this computer…'), false);
        let boxes = null;
        try {
            // The picture as framed, without the marks: nothing drawn so far hides anything from the reading
            const picture = await captureFrame(ui, []);
            if (cancellable.is_cancelled())
                return;
            if (picture)
                boxes = checked(await this._read(picture.bytes, cancellable), picture.width, picture.height);
        } catch (e) {
            if (e.matches?.(Gio.IOErrorEnum, Gio.IOErrorEnum.CANCELLED) || cancellable.is_cancelled())
                return;
            // Never the reason's words alone in a log: it names nothing of the picture, only that it failed
            console.warn(`Amethystora UI: the screenshot could not be read: ${e.message}`);
        } finally {
            if (this._cancellable === cancellable)
                this._cancellable = null;
        }
        if (cancellable.is_cancelled())
            return;
        if (!boxes) {
            // Boxes from an earlier reading would claim what this one could not say
            this.uncover();
            this._show(_('The picture could not be read, so nothing was covered.'), false);
            return;
        }
        this._cover(boxes, x, y, scale);
    }

    async _read(bytes, cancellable) {
        const process = Gio.Subprocess.new(HELPER,
            Gio.SubprocessFlags.STDIN_PIPE | Gio.SubprocessFlags.STDOUT_PIPE | Gio.SubprocessFlags.STDERR_SILENCE);
        // A reading that is cancelled (the bar closes, the mode changes) ends the helper, which takes Tesseract with it
        const cancelled = cancellable.connect(() => process.send_signal(15));
        const stop = GLib.timeout_add(GLib.PRIORITY_DEFAULT, READ_MS, () => {
            process.force_exit();
            return GLib.SOURCE_REMOVE;
        });
        try {
            const [out] = await process.communicate_async(bytes, cancellable);
            return JSON.parse(new TextDecoder().decode(out.toArray()));
        } finally {
            GLib.source_remove(stop);
            cancellable.disconnect(cancelled);
        }
    }

    _cover(boxes, x, y, scale) {
        const counts = {};
        const before = this._marks;
        this._marks = [];
        if (before.length)
            this._session.removeMarks(before);
        this._marks = boxes.map(box => {
            counts[box.kind] = (counts[box.kind] ?? 0) + 1;
            const mark = newMark('fill', BLACK, 1, [x + box.x / scale, y + box.y / scale]);
            mark.points.push([x + (box.x + box.w) / scale, y + (box.y + box.h) / scale]);
            return mark;
        });
        this._session.addMarks(this._marks);
        if (this._marks.length)
            this._show(sentence(counts), true, lines(counts));
        else
            this._show(_('Nothing found to cover. It can miss things, so check before you send.'), false);
    }

    // The shutter, Copy or Save asked for while the picture is still being read would take it without its boxes
    wait() {
        this._show(_('Still reading the picture. Take it once the boxes are on.'), false);
    }

    // Another card takes its place; the boxes stay
    closeCard() {
        this._card?.destroy();
        this._card = null;
    }

    // The boxes off, the card gone and a reading in progress ended: also where the picture can no longer be covered
    // (a window, a recording), so that no card says "covered" over a picture whose boxes are not burnt in
    uncover() {
        this._cancellable?.cancel();
        const marks = this._marks;
        this._marks = [];
        this.closeCard();
        if (marks.length)
            this._session.removeMarks(marks);
    }

    _show(text, covered, details = []) {
        this._card?.destroy();
        const card = new St.BoxLayout({
            style_class: 'amethystora-screenshot-cover-card',
            orientation: Clutter.Orientation.VERTICAL,
            reactive: true,
        });
        const label = new St.Label({style_class: 'amethystora-screenshot-cover-text', text});
        label.clutter_text.use_markup = false;
        label.clutter_text.line_wrap = true;
        card.add_child(label);
        if (details.length) {
            const kinds = new St.Label({style_class: 'amethystora-screenshot-cover-text', text: details.join('\n')});
            kinds.clutter_text.use_markup = false;
            card.add_child(kinds);
        }
        const buttons = new St.BoxLayout({style_class: 'amethystora-screenshot-cover-buttons', x_align: Clutter.ActorAlign.END});
        const button = (words, action, primary = false) => {
            const each = new St.Button({
                style_class: `amethystora-screenshot-cover-button${primary ? ' amethystora-screenshot-cover-primary' : ''}`,
                label: words,
                can_focus: true,
            });
            each.connect('clicked', action);
            buttons.add_child(each);
        };
        if (covered) {
            button(_('Uncover'), () => this.uncover());
            button(_('Copy'), () => this._session.copy(), true);
        } else if (!this.reading) {
            button(_('Close'), () => {
                this._card?.destroy();
                this._card = null;
            });
        }
        if (buttons.get_n_children())
            card.add_child(buttons);
        card.connect('destroy', () => {
            if (this._card === card)
                this._card = null;
        });
        this._overlay.add_child(card);
        this._card = card;
        // Under the toolbar, on the primary screen
        const monitor = Main.layoutManager.primaryMonitor;
        card.set_position(Math.round(monitor.x + (monitor.width - CARD_WIDTH) / 2), monitor.y + CARD_TOP);
    }
}
