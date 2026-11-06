// The agent's own part of the live pill (R7-11, R7-12): a question typed into the pill's card, answered in the same card by the
// chosen agent with Amethystora's read tools alone, and the questions the agent's tools ask the person before they run a command.
//
// Ask right there. The key (`ask-here`, Super+/) puts "Ask <agent>" in the pill with its card open and a field in it. Enter hands the
// question to `amethystora-agent ask-here` on standard input, which starts the agent without a terminal and without any tool but the
// five that read; the answer comes back on standard output and is shown as text. The pill says which agent answers and that the words
// went to its maker. Nothing is written by this file, and everything it holds (the question, the answer) is dropped when the card is
// dismissed, ten minutes later, or when the extension stops (a lock stops it).
//
// Yes or no. A hand-off (a command the agent asks to run, a setting that asks something) is put to the person by the tool server
// over the session bus: `org.amethystora.Agent.Ask(id)` on this shell, which replies with the person's click. No file stands for a yes,
// so nothing an agent can write answers for the person. A root command's password is still asked, by sudo in the terminal that opens
// after an Allow, and a command too long to read in a card is not asked here at all (the terminal prints it whole).

import Clutter from 'gi://Clutter';
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import Pango from 'gi://Pango';
import St from 'gi://St';

import {gettext as _} from 'resource:///org/gnome/shell/extensions/extension.js';

import {buttonRow, cardButton, retire} from '../pill/clock.js';
import {ANSWER_KEPT, QUESTION_KEPT, askInput, isId, outcome, parseQuestion} from './askmodel.js';

const AGENT = 'amethystora-agent';
const PATH = '/org/amethystora/Agent';
const INTERFACE = `
<node>
  <interface name="org.amethystora.Agent">
    <method name="Ask">
      <arg type="s" direction="in" name="id"/>
      <arg type="b" direction="out" name="allowed"/>
    </method>
  </interface>
</node>`;
const ERROR = 'org.amethystora.Agent.Unavailable';

Gio._promisify(Gio.Subprocess.prototype, 'communicate_utf8_async');
Gio._promisify(Gio.File.prototype, 'load_contents_async');

export class AgentAsk {
    // host: the agent module: `pill`, `agentName`, `gicon`, `changes` (the pill's list of what the tools do, which may be null)
    constructor(host) {
        this._host = host;
        this.id = 'ask';
        this.plate = 'agent';
        this._mode = null;
        this._questions = [];
        this._typed = '';
    }

    stop() {
        this.unexport();
        this._cancel();
        for (const question of [...this._questions])
            this._reply(question, false);
        this._mode = null;
        this._typed = '';
        this._answer = null;
        if (this._kept)
            GLib.source_remove(this._kept);
        this._kept = 0;
        this._sync();
    }

    // --- D-Bus: the hand-off's question ------------------------------------------------------------------------------

    export() {
        if (this._dbus)
            return;
        this._dbus = Gio.DBusExportedObject.wrapJSObject(INTERFACE, this);
        this._dbus.export(Gio.DBus.session, PATH);
    }

    unexport() {
        this._dbus?.unexport();
        this._dbus = null;
    }

    // The tool server asks, by the id of its hand-off: the question is in a file of the account's own runtime folder, written by
    // the server, so that the command a person reads is never made of a bus message. The reply is the person's click.
    async AskAsync([id], invocation) {
        if (!this._host.on || !isId(id)) {
            invocation.return_dbus_error(ERROR, 'not asked here');
            return;
        }
        const file = Gio.File.new_for_path(GLib.build_filenamev([GLib.get_user_runtime_dir(), 'amethystora', 'agent-tools', id, 'ask.json']));
        let question;
        try {
            const [bytes] = await file.load_contents_async(null);
            question = parseQuestion(new TextDecoder().decode(bytes));
        } catch {
            question = null;
        }
        // The extension may have stopped while the file was read
        if (!this._dbus || !this._host.on) {
            invocation.return_dbus_error(ERROR, 'not asked here');
            return;
        }
        if (!question || question.tooLong) {
            invocation.return_dbus_error(ERROR, 'not shown here');
            return;
        }
        question.id = id;
        question.invocation = invocation;
        // The caller going away (its server ended) takes the question back
        question.watch = Gio.bus_watch_name_on_connection(Gio.DBus.session, invocation.get_sender(), Gio.BusNameWatcherFlags.NONE, null,
            () => this._reply(question, false, true));
        question.timer = GLib.timeout_add_seconds(GLib.PRIORITY_DEFAULT, QUESTION_KEPT, () => {
            question.timer = 0;
            this._reply(question, false);
            return GLib.SOURCE_REMOVE;
        });
        this._questions.push(question);
        this._sync();
    }

    // The person's answer (or no one's: `gone`), sent back, and the question off the card
    _reply(question, allowed, gone = false) {
        const at = this._questions.indexOf(question);
        if (at < 0)
            return;
        this._questions.splice(at, 1);
        if (question.timer)
            GLib.source_remove(question.timer);
        question.timer = 0;
        if (question.watch)
            Gio.bus_unwatch_name(question.watch);
        question.watch = 0;
        if (!gone) {
            try {
                question.invocation.return_value(new GLib.Variant('(b)', [allowed]));
            } catch (e) {
                console.warn(`Amethystora UI: the agent's question could not be answered: ${e.message}`);
            }
        }
        question.invocation = null;
        this._sync();
    }

    // --- Ask right there ---------------------------------------------------------------------------------------------

    // The key: the field, in the pill's card. Does nothing while the question is being answered (the card shows how it goes).
    open() {
        if (!this._host.on)
            return false;
        if (!this._host.pill?.canAsk?.())
            return false;
        if (this._mode === null || this._mode === 'answer' || this._mode === 'failed') {
            this._mode = 'typing';
            this._answer = null;
        }
        this._sync();
        this._host.pill.openCard('ask');
        return true;
    }

    // A question with what it is about (text, or a picture in the private folder), from the key or from a drop (R7-14)
    async ask(question, {quoted = '', picture = '', onSent = null} = {}) {
        const text = question.replace(/\s+/g, ' ').trim();
        if (!text || this._process)
            return;
        this._mode = 'working';
        this._answer = null;
        this._typed = '';
        this._lastQuestion = text;
        this._quiet(true);
        this._sync();
        const cancellable = new Gio.Cancellable();
        this._cancellable = cancellable;
        let result;
        try {
            const process = Gio.Subprocess.new([AGENT, 'ask-here'],
                Gio.SubprocessFlags.STDIN_PIPE | Gio.SubprocessFlags.STDOUT_PIPE | Gio.SubprocessFlags.STDERR_SILENCE);
            this._process = process;
            onSent?.();
            const [stdout] = await process.communicate_utf8_async(askInput(text, {quoted, picture}), cancellable);
            result = outcome(process.get_if_exited() ? process.get_exit_status() : -1, stdout ?? '');
        } catch {
            result = cancellable.is_cancelled() ? null : {kind: 'failed', why: 'error', detail: ''};
        }
        // Stopped, or the extension went: nothing to show
        if (this._cancellable !== cancellable)
            return;
        this._cancellable = null;
        this._process = null;
        this._quiet(false);
        if (!result) {
            this._mode = null;
        } else {
            this._mode = result.kind;
            this._answer = result;
            if (this._kept)
                GLib.source_remove(this._kept);
            // The answer is not kept for good: it goes after a while, and with it the question
            this._kept = GLib.timeout_add_seconds(GLib.PRIORITY_DEFAULT, ANSWER_KEPT, () => {
                this._kept = 0;
                this._dismiss();
                return GLib.SOURCE_REMOVE;
            });
        }
        this._sync();
    }

    // The tools' own line about what is read stays out of the pill while this one says it
    _quiet(on) {
        const changes = this._host.changes;
        if (changes)
            changes.hush = on;
    }

    // What the tool server is doing changed (the pill's line is this activity's while it works)
    noted() {
        if (this._mode === 'working')
            this._sync();
    }

    _cancel() {
        const cancellable = this._cancellable;
        this._cancellable = null;
        cancellable?.cancel();
        // SIGTERM to the launcher, which `timeout` hands on to the agent
        try {
            this._process?.send_signal(15);
        } catch {}
        this._process = null;
        this._quiet(false);
    }

    _dismiss() {
        this._cancel();
        if (this._kept)
            GLib.source_remove(this._kept);
        this._kept = 0;
        this._mode = null;
        this._answer = null;
        this._typed = '';
        this._lastQuestion = '';
        this._sync();
    }

    // --- The pill's activity ---------------------------------------------------------------------------------------------

    get active() {
        return this._mode !== null || this._questions.length > 0;
    }

    get text() {
        const name = this._host.agentName;
        if (this._questions.length > 0) {
            const first = this._questions[0];
            return first.root ? _('Agent asks for root') : _('Agent asks you');
        }
        switch (this._mode) {
        case 'typing': return _('Ask %s').format(name);
        case 'working': return this._host.changes?.doingText ?? _('%s is looking…').format(name);
        case 'answer': return _('%s answered').format(name);
        case 'failed': return _('No answer');
        default: return '';
        }
    }

    get accessible() {
        return this.text;
    }

    get row() {
        return {icon: this._host.gicon, text: this.text};
    }

    pill() {
        if (!this._content) {
            this._content = new St.BoxLayout({style_class: 'amethystora-pill-content amethystora-agent-pill'});
            this._content.add_child(new St.Icon({gicon: this._host.gicon, icon_size: 13, y_align: Clutter.ActorAlign.CENTER}));
            this._label = new St.Label({text: this.text, y_align: Clutter.ActorAlign.CENTER});
            this._label.clutter_text.use_markup = false;
            this._content.add_child(this._label);
        }
        return this._content;
    }

    _sync() {
        const pill = this._host.pill;
        const active = this.active;
        this.plate = this._questions.length > 0 ? 'agent-ask' : this._mode === 'answer' ? 'agent-done' : 'agent';
        if (active !== !!this._shown) {
            this._shown = active;
            pill?.setAsk?.(active ? this : null);
            if (!active) {
                if (this._content)
                    pill?.clock?.forget(this._content);
                retire(this._content);
                this._content = null;
                this._label = null;
            }
        }
        if (active) {
            if (this._label)
                this._label.text = this.text;
            pill?.askChanged?.();
            this._fill();
        }
    }

    card(api) {
        this._api = api;
        const body = new St.BoxLayout({style_class: 'amethystora-pill-card-body amethystora-agent-card', orientation: Clutter.Orientation.VERTICAL});
        this._body = body;
        body.connect('destroy', () => {
            if (this._body !== body)
                return;
            this._body = null;
            // The pill's own card refresh makes a new body at once; a card that was shut does not
            GLib.idle_add(GLib.PRIORITY_DEFAULT_IDLE, () => {
                if (!this._body && this._mode === 'typing')
                    this._dismiss();
                return GLib.SOURCE_REMOVE;
            });
        });
        this._fill();
        return body;
    }

    _fill() {
        const body = this._body;
        if (!body)
            return;
        // What was typed is kept across a refill, which the pill's card does whenever another activity comes or goes
        if (this._entry && this._mode === 'typing')
            this._typed = this._entry.text;
        body.destroy_all_children();
        this._entry = null;
        for (const question of this._questions)
            body.add_child(this._questionBlock(question));
        if (this._mode)
            body.add_child(this._askBlock());
    }

    // --- The cards ---------------------------------------------------------------------------------------------------------

    _head(title, who) {
        const head = new St.BoxLayout({style_class: 'amethystora-agent-card-head'});
        head.add_child(new St.Label({style_class: 'amethystora-agent-card-title', text: title, x_expand: true, y_align: Clutter.ActorAlign.END}));
        if (who)
            head.add_child(new St.Label({style_class: 'amethystora-agent-card-who', text: who, y_align: Clutter.ActorAlign.END}));
        return head;
    }

    _text(text, style, {mono = false} = {}) {
        const label = new St.Label({style_class: style, text, x_expand: true});
        label.clutter_text.use_markup = false;
        label.clutter_text.line_wrap = true;
        label.clutter_text.line_wrap_mode = Pango.WrapMode.WORD_CHAR;
        label.clutter_text.ellipsize = Pango.EllipsizeMode.NONE;
        // A command and an answer about files and logs read left to right in any language
        if (mono)
            label.text_direction = Clutter.TextDirection.LTR;
        return label;
    }

    // One question from the agent's tools: the exact command, the agent's reason as its own words, Allow and Not now
    _questionBlock(question) {
        const block = new St.BoxLayout({style_class: 'amethystora-agent-act amethystora-agent-act-ask amethystora-agent-question', orientation: Clutter.Orientation.VERTICAL});
        const title = question.kind === 'change'
            ? _('May I change a setting?')
            : question.root ? _('May I run this as root?') : _('May I run this?');
        block.add_child(this._head(title, this._host.agentName));
        if (question.title)
            block.add_child(this._text(question.title, 'amethystora-agent-act-what'));
        block.add_child(this._text(question.root ? `sudo -- ${question.line}` : question.line, 'amethystora-agent-command', {mono: true}));
        if (question.reason)
            block.add_child(this._text(_('Its reason, in its own words: %s').format(question.reason), 'amethystora-agent-act-how'));
        block.add_child(this._text(question.root
            ? _('It runs in a terminal, where sudo asks for your password.')
            : _('It runs in a terminal, and what it prints goes back to the agent.'), 'amethystora-agent-act-how'));
        block.add_child(buttonRow([
            cardButton(_('Not now'), () => this._reply(question, false), {small: true}),
            cardButton(_('Allow'), () => this._reply(question, true), {small: true, primary: true}),
        ]));
        return block;
    }

    // The question typed, how it goes, and the answer
    _askBlock() {
        const box = new St.BoxLayout({style_class: 'amethystora-agent-ask-part', orientation: Clutter.Orientation.VERTICAL});
        const name = this._host.agentName;
        const buttons = [];
        switch (this._mode) {
        case 'typing': {
            box.add_child(this._head(_('Ask %s').format(name)));
            const entry = new St.Entry({style_class: 'amethystora-agent-field', hint_text: _('What do you want to know?'), can_focus: true, x_expand: true});
            entry.clutter_text.max_length = 600;
            entry.text = this._typed;
            entry.clutter_text.connect('activate', () => this.ask(entry.text));
            this._entry = entry;
            box.add_child(entry);
            box.add_child(this._text(_('It can look at this computer, and it changes nothing. Your words go to the maker of %s.').format(name), 'amethystora-agent-act-how'));
            buttons.push(cardButton(_('Cancel'), () => this._leave(), {small: true}),
                cardButton(_('Ask'), () => this.ask(entry.text), {small: true, primary: true}));
            GLib.idle_add(GLib.PRIORITY_DEFAULT_IDLE, () => {
                if (this._entry === entry && entry.get_stage())
                    entry.grab_key_focus();
                return GLib.SOURCE_REMOVE;
            });
            break;
        }
        case 'working':
            box.add_child(this._head(_('%s is looking…').format(name)));
            box.add_child(this._text(this._host.changes?.doingText ?? _('It can look at this computer, and it changes nothing.'), 'amethystora-agent-act-how'));
            buttons.push(cardButton(_('Stop'), () => this._dismiss(), {small: true}));
            break;
        case 'answer':
            box.add_child(this._head(_('%s answered').format(name)));
            box.add_child(this._text(this._answer.text, 'amethystora-agent-answer', {mono: true}));
            box.add_child(this._text(_('Your question left this computer, for the maker of %s.').format(name), 'amethystora-agent-act-how'));
            buttons.push(cardButton(_('Copy'), () => this._copy(), {small: true}),
                cardButton(_('Close'), () => this._leave(), {small: true, primary: true}));
            break;
        default:
            box.add_child(this._head(_('No answer')));
            box.add_child(this._text(this._failure(), 'amethystora-agent-act-how'));
            if (this._answer?.why === 'headless')
                buttons.push(cardButton(_('Open it in a terminal'), () => this._terminal(), {small: true}));
            buttons.push(cardButton(_('Close'), () => this._leave(), {small: true, primary: true}));
        }
        const row = buttonRow(buttons);
        row.add_style_class_name('amethystora-agent-card-buttons');
        box.add_child(row);
        return box;
    }

    _failure() {
        const name = this._host.agentName;
        const why = this._answer?.why;
        switch (why) {
        case 'headless': return _('%s has no way here to answer without changing anything. It can open in a terminal instead.').format(name);
        case 'missing': return _('%s is not installed yet. Open it once in a terminal to install it.').format(name);
        case 'off': return _('The agent is off for this account.');
        case 'slow': return _('%s took too long to answer, so it was stopped.').format(name);
        case 'empty': return _('%s gave no answer.').format(name);
        default: return this._answer?.detail || _('%s could not answer.').format(name);
        }
    }

    // Closing the card: an answer is let go of, a field with nothing typed too
    _leave() {
        this._api?.close();
        this._dismiss();
    }

    _copy() {
        if (this._answer?.text)
            St.Clipboard.get_default().set_text(St.ClipboardType.CLIPBOARD, this._answer.text);
    }

    // The agent in a terminal, with what was asked as `ask` takes it (the person's own words, never one starting with "-")
    _terminal() {
        const question = this._lastQuestion;
        this._leave();
        if (question && !question.startsWith('-')) {
            try {
                Gio.Subprocess.new([AGENT, 'ask', '--', question], Gio.SubprocessFlags.STDOUT_SILENCE | Gio.SubprocessFlags.STDERR_SILENCE);
            } catch (e) {
                console.warn(`Amethystora UI: the agent could not be opened: ${e.message}`);
            }
        }
    }

    // For the suites
    state() {
        return {mode: this._mode, questions: this._questions.map(q => ({id: q.id, kind: q.kind, root: q.root, line: q.line})),
            text: this.active ? this.text : '', answer: this._answer?.text ?? null, shown: !!this._shown, exported: !!this._dbus,
            card: !!this._body, field: this._entry?.text ?? null};
    }
}
