// Reading the selection aloud: the text selected anywhere (the primary selection) read on this computer with the voice
// the screen reader uses, Speech Dispatcher and eSpeak NG, which the image already has for Orca. Nothing is sent
// anywhere and nothing is kept. Sentence by sentence, one spd-say at a time, so that the speed can change at the next
// sentence and Stop is at once. Each sentence goes to spd-say on its standard input (its pipe mode), never on its
// command line, which every account on the machine can read, and never as a shell's words or an option.
//
// While it reads, the live pill says so with the speed and Stop (an activity of the pill's, 'reading').

import Clutter from 'gi://Clutter';
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import St from 'gi://St';

import {gettext as _} from 'resource:///org/gnome/shell/extensions/extension.js';

import {buttonRow, cardButton, chip, retire} from '../pill/clock.js';
import {lineIcon} from './lines.js';

// The board's speaker, on a square of 24
const SPEAKER = ['M4 10v4h4l5 4V6L8 10zM16 9a4 4 0 0 1 0 6M18.5 6.5a8 8 0 0 1 0 11'];
// More than this is not read
const LIMIT = 20000;
// The speeds the chip steps through, and Speech Dispatcher's rate for each (−100 to 100, 0 the voice's own)
export const SPEEDS = ['1', '1.5', '2', '0.75'];
const RATES = {'0.75': -30, '1': 0, '1.5': 40, '2': 75};
const SPD_SAY = 'spd-say';

// The session's language as Speech Dispatcher names it, or null for its own default
function language() {
    for (const name of GLib.get_language_names()) {
        const code = name.split(/[_.@]/)[0];
        if (/^[a-z]{2,3}$/.test(code))
            return code;
    }
    return null;
}

// The selection as sentences: at most LIMIT characters (cut first, and never inside a character), control characters
// but line breaks and tabs taken out
export function sentences(text, lang) {
    const cut = text.length > LIMIT;
    let kept = cut ? text.slice(0, LIMIT) : text;
    if (/[\uD800-\uDBFF]$/.test(kept))
        kept = kept.slice(0, -1);
    // eslint-disable-next-line no-control-regex
    kept = kept.replace(/[\u0000-\u0008\u000b\u000c\u000e-\u001f\u007f-\u009f]/g, ' ');
    let parts;
    try {
        const segmenter = new Intl.Segmenter(lang ?? undefined, {granularity: 'sentence'});
        parts = [...segmenter.segment(kept)].map(s => s.segment);
    } catch {
        parts = kept.split(/(?<=[.!?…])\s+/);
    }
    return {parts: parts.map(p => p.trim()).filter(p => p.length > 0), cut};
}

export class Speech {
    constructor(access) {
        this._access = access;
        this._queue = null;
        this._process = null;
        this._cancellable = null;
    }

    get reading() {
        return this._queue !== null;
    }

    // The key: read the selection, or stop when reading
    toggle() {
        if (this.reading) {
            this.stop();
            return;
        }
        St.Clipboard.get_default().get_text(St.ClipboardType.PRIMARY, (clipboard, text) => {
            if (!this._access.enabled)
                return;
            const lang = language();
            const {parts, cut} = sentences(text ?? '', lang);
            if (parts.length === 0) {
                this._access.pill?.notice?.(_('Select some text first'), 'edit-select-all-symbolic', 3);
                return;
            }
            this._start(parts, cut, lang);
        });
    }

    _start(parts, cut, lang) {
        this.stop();
        this._queue = parts;
        this._cut = cut;
        this._lang = lang;
        this._cancellable = new Gio.Cancellable();
        this._access.pill?.setReading?.(this);
        this._next();
    }

    _next() {
        if (!this._queue || this._queue.length === 0) {
            this._finish();
            return;
        }
        // One line, the sentence's own line breaks spaces; a line pipe mode would take as a command of its own is
        // made plain text
        const sentence = this._queue.shift().replace(/\s+/g, ' ').replace(/^!-!/, '! - !');
        const argv = [SPD_SAY, '--pipe-mode', '--wait', '--application-name', 'amethystora',
            '--rate', `${RATES[this.speed] ?? 0}`];
        if (this._lang)
            argv.push('--language', this._lang);
        let process;
        try {
            process = Gio.Subprocess.new(argv, Gio.SubprocessFlags.STDIN_PIPE | Gio.SubprocessFlags.STDOUT_SILENCE |
                Gio.SubprocessFlags.STDERR_SILENCE);
        } catch (e) {
            console.warn(`Amethystora UI: reading aloud did not start: ${e.message}`);
            this._finish();
            return;
        }
        this._process = process;
        const cancellable = this._cancellable;
        // The sentence written, the pipe closed; done when spd-say has said it and ended
        process.communicate_utf8_async(`${sentence}\n`, cancellable, (p, result) => {
            try {
                p.communicate_utf8_finish(result);
            } catch {
                return;
            }
            if (this._process !== p || cancellable.is_cancelled())
                return;
            this._process = null;
            if (!p.get_successful()) {
                this._finish();
                return;
            }
            this._next();
        });
    }

    // Stops now: the sentence being said, and the rest
    stop() {
        if (!this.reading)
            return;
        this._cancellable?.cancel();
        this._cancellable = null;
        if (this._process) {
            this._process.force_exit();
            this._process = null;
            // What Speech Dispatcher is saying now stops too. spd-say's stop is for the message being spoken, whoever
            // sent it: Orca's sentence of that moment stops as well (the manual says so)
            try {
                Gio.Subprocess.new([SPD_SAY, '--stop'], Gio.SubprocessFlags.STDOUT_SILENCE | Gio.SubprocessFlags.STDERR_SILENCE);
            } catch {}
        }
        this._finish();
    }

    _finish() {
        this._queue = null;
        this._process = null;
        this._cancellable = null;
        this._access.pill?.setReading?.(null);
        this.destroyContent();
    }

    get speed() {
        const speed = this._access.settings?.get_string('read-aloud-speed') ?? '1';
        return SPEEDS.includes(speed) ? speed : '1';
    }

    // The next speed, kept for next time; it applies from the next sentence
    nextSpeed() {
        const next = SPEEDS[(SPEEDS.indexOf(this.speed) + 1) % SPEEDS.length];
        this._access.settings?.set_string('read-aloud-speed', next);
    }

    speedChanged() {
        if (this._speedChip) {
            this._speedChip.text = speedName(this.speed);
            this._access.pill?.readingChanged?.();
        }
    }

    // ---- The pill's activity ----

    get id() {
        return 'reading';
    }

    get plate() {
        return '';
    }

    get text() {
        return _('Reading aloud');
    }

    get accessible() {
        return _('Reading the selection aloud');
    }

    get row() {
        return {icon: 'audio-speakers-symbolic', text: _('Reading aloud')};
    }

    pill() {
        if (!this._content) {
            const content = new St.BoxLayout({style_class: 'amethystora-pill-content amethystora-access-reading'});
            content.add_child(lineIcon(SPEAKER, 'amethystora-access-speaker'));
            const label = new St.Label({text: this.text, y_align: Clutter.ActorAlign.CENTER});
            label.clutter_text.use_markup = false;
            content.add_child(label);
            this._speedChip = chip(speedName(this.speed), () => this.nextSpeed());
            content.add_child(this._speedChip);
            content.add_child(chip(_('Stop'), () => this.stop()));
            this._content = content;
        }
        return this._content;
    }

    card(api) {
        const body = new St.BoxLayout({style_class: 'amethystora-pill-card-body', orientation: Clutter.Orientation.VERTICAL});
        body.add_child(new St.Label({style_class: 'amethystora-pill-card-label', text: _('Reading aloud')}));
        const words = [_('The selection is read on this computer; nothing is sent anywhere.')];
        if (this._cut)
            words.push(_('Only the first part of a long selection is read.'));
        const text = new St.Label({style_class: 'amethystora-pill-card-text', text: words.join('\n')});
        text.clutter_text.line_wrap = true;
        body.add_child(text);
        body.add_child(buttonRow([cardButton(_('Stop'), () => {
            api.close();
            this.stop();
        }, {primary: true})]));
        return body;
    }

    destroyContent() {
        retire(this._content);
        this._content = null;
        this._speedChip = null;
    }

    destroy() {
        this.stop();
        this.destroyContent();
    }

    state() {
        return {reading: this.reading, left: this._queue?.length ?? 0, process: !!this._process, speed: this.speed};
    }
}

export function speedName(speed) {
    switch (speed) {
    case '0.75':
        return _('0.75×');
    case '1.5':
        return _('1.5×');
    case '2':
        return _('2×');
    default:
        return _('1×');
    }
}
