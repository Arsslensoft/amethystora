// Ask about a selection: the text selected in any window (the primary selection, which every app sets as text is
// selected) is read, at most READ bytes of it and through Mutter's own transfer with a size, so that a whole book
// selected costs no more than a page; then a card shows the start of it with a field for the question, Cancel and Ask.
// Ask hands both to `amethystora-agent ask --quoted --question QUESTION -- TEXT`, which gives the text to the agent as
// data between bars, never as the question, and opens the agent in a terminal. Nothing is read before the key, nothing
// is kept after the card closes, and nothing at all happens while the agentic features are off (the key is not bound).

import Clutter from 'gi://Clutter';
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import Meta from 'gi://Meta';
import Pango from 'gi://Pango';
import St from 'gi://St';

import {gettext as _} from 'resource:///org/gnome/shell/extensions/extension.js';
import * as ModalDialog from 'resource:///org/gnome/shell/ui/modalDialog.js';

import {inCloseDialog} from '../quit/frozen.js';

const AGENT = 'amethystora-agent';
// Bytes read of the selection, and characters handed on (amethystora-agent cuts there too)
const READ = 16384;
export const LONGEST = 4000;
// Lines of the selection the card shows
const SHOWN = 4;
// How long an app that owns the selection has to hand it over
const PATIENCE = 2000;
// The selection's text types, in the order they are asked for
const TYPES = ['text/plain;charset=utf-8', 'UTF8_STRING', 'text/plain', 'STRING'];
// C0 and C1 control characters but the newline and the tab, and the characters that turn text around
const CONTROLS = /[\u0000-\u0008\u000b-\u001f\u007f-\u009f‎‏؜‪-‮⁦-⁩]/gu;

// The selected text, cleaned and cut to LONGEST characters, or '' when nothing is selected or it is not text
export function readSelection(cancellable) {
    return new Promise(resolve => {
        const selection = global.display.get_selection();
        const offered = selection.get_mimetypes(Meta.SelectionType.SELECTION_PRIMARY);
        const type = TYPES.find(t => offered.includes(t));
        if (!type) {
            resolve('');
            return;
        }
        const stream = Gio.MemoryOutputStream.new_resizable();
        let timer = GLib.timeout_add(GLib.PRIORITY_DEFAULT, PATIENCE, () => {
            timer = 0;
            cancellable.cancel();
            return GLib.SOURCE_REMOVE;
        });
        selection.transfer_async(Meta.SelectionType.SELECTION_PRIMARY, type, READ, stream, cancellable, (s, result) => {
            if (timer)
                GLib.source_remove(timer);
            try {
                s.transfer_finish(result);
                stream.close(null);
                resolve(clean(new TextDecoder().decode(stream.steal_as_bytes().toArray())));
            } catch {
                resolve('');
            }
        });
    });
}

// Text somebody else wrote, made fit to show and hand on: no control characters, tabs as spaces, CRLF and the other
// line breaks (NEL, U+2028, U+2029) as LF, no half character left where the read stopped, at most LONGEST characters
export function clean(text) {
    const chars = [...text.replace(/\r\n?|[\u0085\u2028\u2029]/g, '\n').replace(/\t/g, ' ').replace(CONTROLS, '').replace(/�+$/u, '')];
    return chars.slice(0, LONGEST).join('').trim();
}

// The first SHOWN lines, each cut to fit, with … where more follows
function preview(text) {
    const lines = text.split('\n');
    const shown = lines.slice(0, SHOWN).map(line => (line.length > 160 ? `${line.slice(0, 160)}…` : line));
    if (lines.length > SHOWN)
        shown[SHOWN - 1] += ' …';
    return shown.join('\n');
}

export class AskCard {
    // agentTitle: the chosen agent's name, for the line under the card; gicon: the agent's mark; closed() when it has gone
    constructor(text, agentTitle, gicon, closed) {
        this._text = text;
        this._closed = closed;
        this._dialog = new ModalDialog.ModalDialog({styleClass: 'amethystora-agent-ask', destroyOnClose: true});
        this._dialog.connect('closed', () => {
            global.stage.disconnectObject(this);
            this._dialog = null;
            this._closed?.();
        });
        this._build(agentTitle, gicon);
    }

    _build(agentTitle, gicon) {
        const box = new St.BoxLayout({style_class: 'amethystora-agent-ask-box', orientation: Clutter.Orientation.VERTICAL, x_expand: true});
        const head = new St.BoxLayout({style_class: 'amethystora-agent-ask-head'});
        head.add_child(new St.Icon({
            style_class: 'amethystora-agent-ask-icon',
            gicon,
            icon_size: 16,
            y_align: Clutter.ActorAlign.CENTER,
        }));
        head.add_child(new St.Label({style_class: 'amethystora-agent-ask-title', text: _('Ask about the selected text'), y_align: Clutter.ActorAlign.CENTER}));
        box.add_child(head);

        const quote = new St.Label({style_class: 'amethystora-agent-ask-quote', text: preview(this._text), x_expand: true});
        quote.clutter_text.use_markup = false;
        quote.clutter_text.line_wrap = true;
        quote.clutter_text.line_wrap_mode = Pango.WrapMode.WORD_CHAR;
        quote.clutter_text.ellipsize = Pango.EllipsizeMode.NONE;
        // What was selected reads left to right whatever the session's language: code, paths, logs
        quote.text_direction = Clutter.TextDirection.LTR;
        box.add_child(quote);

        this._entry = new St.Entry({
            style_class: 'amethystora-agent-ask-field',
            hint_text: _('What do you want to know?'),
            can_focus: true,
            x_expand: true,
        });
        this._entry.clutter_text.max_length = 600;
        this._entry.clutter_text.connect('activate', () => this.ask());
        box.add_child(this._entry);
        this._dialog.contentLayout.add_child(box);

        // The buttons sit at the end, the line saying where the answer opens at the start
        const buttons = this._dialog.buttonLayout;
        buttons.layout_manager.homogeneous = false;
        buttons.add_style_class_name('amethystora-agent-ask-buttons');
        const hint = new St.Label({
            style_class: 'amethystora-agent-ask-hint',
            text: _('Opens %s in a terminal').format(agentTitle),
            x_expand: true,
            y_align: Clutter.ActorAlign.CENTER,
        });
        buttons.add_child(hint);
        this._cancel = this._dialog.addButton({label: _('Cancel'), action: () => this.close(), key: Clutter.KEY_Escape});
        this._ask = this._dialog.addButton({label: _('Ask'), action: () => this.ask()});
        for (const button of [this._cancel, this._ask]) {
            button.x_expand = false;
            button.add_style_class_name('amethystora-agent-ask-button');
        }
        this._ask.add_style_class_name('amethystora-agent-ask-primary');
        this._dialog.setInitialKeyFocus(this._entry);
    }

    open() {
        if (!this._dialog.open()) {
            // Another grab holds the keyboard: nothing opened
            const closed = this._closed;
            this.destroy();
            closed?.();
            return;
        }
        // The keyboard stays in the field while the card is open: GNOME's own not-responding question takes the key
        // focus into its window whenever Mutter asks it to, even under a modal dialog. Only from that question is it
        // taken back; a dialog opened over the card (a password prompt) keeps it
        global.stage.connectObject('notify::key-focus', () => {
            if (this._dialog?.state === ModalDialog.State.OPENED && inCloseDialog(global.stage.key_focus))
                this._entry?.grab_key_focus();
        }, this);
    }

    get isOpen() {
        return !!this._dialog;
    }

    get question() {
        return this._entry?.text ?? '';
    }

    // The question and the text go to the agent as two arguments, the text after --; the card closes
    ask() {
        if (!this._dialog)
            return;
        const question = this._entry.text.replace(CONTROLS, ' ').trim();
        const argv = [AGENT, 'ask', '--quoted'];
        if (question)
            argv.push('--question', question);
        argv.push('--', this._text);
        try {
            Gio.Subprocess.new(argv, Gio.SubprocessFlags.STDOUT_SILENCE | Gio.SubprocessFlags.STDERR_SILENCE);
        } catch (e) {
            console.warn(`Amethystora UI: the agent could not be asked: ${e.message}`);
        }
        this.close();
    }

    close() {
        this._dialog?.close();
    }

    // Gone at once (the module stops)
    destroy() {
        const dialog = this._dialog;
        this._dialog = null;
        this._closed = null;
        global.stage.disconnectObject(this);
        // The keyboard and pointer are given back first: a destroyed dialog cannot give them back itself
        dialog?.popModal();
        dialog?.destroy();
    }
}
