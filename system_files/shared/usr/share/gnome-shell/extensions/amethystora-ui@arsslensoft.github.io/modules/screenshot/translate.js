// Any language, on your screen (R7-47; off until a language is chosen, `ame desktop translate`): one button in the
// screenshot bar reads what the box holds and translates it on this computer, into a card. The picture goes to the helper
// through a pipe, and the words come back only to be shown: never logged, never written, gone with the card. The helper's
// two programs end as soon as they have answered, so that nothing stays in memory after a box.

import Clutter from 'gi://Clutter';
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import St from 'gi://St';

import {gettext as _} from 'resource:///org/gnome/shell/extensions/extension.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';

import {captureFrame} from './capture.js';

const HELPER = '/usr/libexec/amethystora-translate';
// A reading runs in a scope of its own, as the cover's does: out of GNOME Shell's cgroup, and in a memory of its own. The
// engine was measured at 253 MB at the most (the limit is 300), and the helper that starts it takes some more
const SCOPE = ['systemd-run', '--user', '--scope', '--quiet', '--collect', '-p', 'MemoryMax=400M', '-p', 'CPUWeight=50'];
// Over the helper's own limits added up (reading 25 s, then 60 s for each of the two steps)
const READ_MS = 150000;
// As wide as stylesheets/screenshot.css draws it, and below the toolbar
const CARD_WIDTH = 460;
const CARD_TOP = 72;

Gio._promisify(Gio.Subprocess.prototype, 'communicate_async');

// The language last read from, for as long as the shell runs: which language a person reads says something, so it is kept
// nowhere else
let lastFrom = null;

async function ask(args, bytes, cancellable) {
    const flags = Gio.SubprocessFlags.STDOUT_PIPE | Gio.SubprocessFlags.STDERR_SILENCE |
        (bytes ? Gio.SubprocessFlags.STDIN_PIPE : 0);
    const process = Gio.Subprocess.new([...(bytes ? SCOPE : []), HELPER, ...args], flags);
    const stop = GLib.timeout_add(GLib.PRIORITY_DEFAULT, READ_MS, () => {
        process.force_exit();
        return GLib.SOURCE_REMOVE;
    });
    // Cancelling ends the helper, which takes its two programs with it (they die with their parent); communicate_async alone
    // only stops waiting for them
    const cancelled = cancellable?.connect(() => process.send_signal(15));
    try {
        const [out] = await process.communicate_async(bytes, cancellable);
        return JSON.parse(new TextDecoder().decode(out.toArray()));
    } finally {
        GLib.source_remove(stop);
        if (cancelled)
            cancellable.disconnect(cancelled);
    }
}

export class Translate {
    // overlay is where the card lives
    constructor(ui, overlay) {
        this._ui = ui;
        this._overlay = overlay;
    }

    destroy() {
        this._cancellable?.cancel();
        this._cancellable = null;
        this.close();
    }

    close() {
        this._card?.destroy();
        this._card = null;
    }

    async run(to, from = null) {
        // A press while a box is being read is ignored: chips pressed in a row would each start an engine
        if (this._cancellable)
            return;
        const cancellable = new Gio.Cancellable();
        this._cancellable = cancellable;
        this._say(_('Reading the picture, on this computer…'), false);
        let answer = null;
        let here = [];
        try {
            const status = await ask(['status', to], null, cancellable);
            here = Array.isArray(status?.from) ? status.from.filter(code => typeof code === 'string') : [];
            const pick = from ?? (here.includes(lastFrom) ? lastFrom : here[0]);
            if (!pick) {
                this._say(_('No language to read is on this computer yet. `ame desktop translate` fetches one.'));
                return;
            }
            lastFrom = pick;
            // The picture as framed, without the marks
            const picture = await captureFrame(this._ui, []);
            if (cancellable.is_cancelled())
                return;
            if (picture)
                answer = await ask(['read', pick, to], picture.bytes, cancellable);
            if (cancellable.is_cancelled())
                return;
            this._show(answer, await this._names(here, cancellable), here, pick, to);
        } catch (e) {
            if (e.matches?.(Gio.IOErrorEnum, Gio.IOErrorEnum.CANCELLED) || cancellable.is_cancelled())
                return;
            // Only that it failed: the reason names nothing of the picture, and no word of it is in this message
            console.warn(`Amethystora UI: the box could not be translated: ${e.message}`);
            this._say(_('This could not be read here.'));
        } finally {
            if (this._cancellable === cancellable)
                this._cancellable = null;
        }
    }

    async _names(here, cancellable) {
        if (here.length < 2)
            return {};
        const list = await ask(['languages'], null, cancellable);
        return Object.fromEntries(list.map(item => [item.code, item.name]));
    }

    _show(answer, names, here, from, to) {
        if (typeof answer?.text !== 'string') {
            this._say(answer?.error === 'no-text'
                ? _('No text was found in the box.')
                : _('This could not be read here.'));
            return;
        }
        const card = this._newCard();
        const words = new St.BoxLayout({orientation: Clutter.Orientation.VERTICAL, style_class: 'amethystora-screenshot-translate-words'});
        words.add_child(this._label(_('Selected from the screen'), 'amethystora-screenshot-translate-title'));
        words.add_child(this._label(String(answer.source ?? ''), 'amethystora-screenshot-translate-source'));
        words.add_child(this._label(answer.text, 'amethystora-screenshot-cover-text'));
        card.add_child(new St.ScrollView({
            style_class: 'amethystora-screenshot-translate-scroll',
            hscrollbar_policy: St.PolicyType.NEVER,
            vscrollbar_policy: St.PolicyType.AUTOMATIC,
            child: words,
        }));
        const note = answer.cut
            ? _('Translated here. Nothing left this computer. The box held more than one card can take, so the end was left out.')
            : _('Translated here. Nothing left this computer.');
        card.add_child(this._label(note, 'amethystora-screenshot-translate-note'));
        if (here.length > 1) {
            const chips = new St.BoxLayout({style_class: 'amethystora-screenshot-cover-buttons'});
            for (const code of here) {
                chips.add_child(this._button(names[code] ?? code, () => this.run(to, code).catch(logError), code === from));
            }
            card.add_child(chips);
        }
        const buttons = new St.BoxLayout({style_class: 'amethystora-screenshot-cover-buttons', x_align: Clutter.ActorAlign.END});
        buttons.add_child(this._button(_('Copy'), () => {
            St.Clipboard.get_default().set_text(St.ClipboardType.CLIPBOARD, answer.text);
        }));
        // The box can be moved and the button pressed again
        buttons.add_child(this._button(_('Translate more'), () => this.close(), true));
        card.add_child(buttons);
        this._place(card);
    }

    _say(text, closes = true) {
        const card = this._newCard();
        card.add_child(this._label(text, 'amethystora-screenshot-cover-text'));
        if (closes) {
            const buttons = new St.BoxLayout({style_class: 'amethystora-screenshot-cover-buttons', x_align: Clutter.ActorAlign.END});
            buttons.add_child(this._button(_('Close'), () => this.close()));
            card.add_child(buttons);
        }
        this._place(card);
    }

    _newCard() {
        this._card?.destroy();
        const card = new St.BoxLayout({
            style_class: 'amethystora-screenshot-cover-card amethystora-screenshot-translate-card',
            orientation: Clutter.Orientation.VERTICAL,
            reactive: true,
        });
        card.connect('destroy', () => {
            if (this._card === card)
                this._card = null;
        });
        this._card = card;
        return card;
    }

    _place(card) {
        this._overlay.add_child(card);
        // Under the toolbar, on the primary screen
        const monitor = Main.layoutManager.primaryMonitor;
        card.set_position(Math.round(monitor.x + (monitor.width - CARD_WIDTH) / 2), monitor.y + CARD_TOP);
    }

    // Plain text whatever the words hold, and only what a person reads: words read off a screen are never markup
    _label(text, styleClass) {
        const label = new St.Label({style_class: styleClass, text});
        label.clutter_text.use_markup = false;
        label.clutter_text.line_wrap = true;
        return label;
    }

    _button(words, action, primary = false) {
        const button = new St.Button({
            style_class: `amethystora-screenshot-cover-button${primary ? ' amethystora-screenshot-cover-primary' : ''}`,
            label: words,
            can_focus: true,
        });
        button.connect('clicked', action);
        return button;
    }
}
