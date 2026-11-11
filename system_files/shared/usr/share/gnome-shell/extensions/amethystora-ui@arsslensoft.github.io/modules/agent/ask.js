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
import {plain} from '../pill/text.js';
import {ANSWER_KEPT, PICTURE_MAX, QUESTION_KEPT, TEXT_READ, aboutFiles, askInput, isId, outcome, parseQuestion, pictureEnd} from './askmodel.js';
// The clipboard history's own header check, a copy of its file (another extension's files cannot be imported; 20-tests.sh holds the two the same)
import {pictureFits} from './pictures.js';

const AGENT = 'amethystora-agent';
const TOOLS = '/usr/libexec/amethystora-agent-tools';
// The start of a picture file read to judge its header: a JPEG's size may come after metadata
const PICTURE_HEAD = 1024 * 1024;
const PATH = '/org/amethystora/Agent';
const INTERFACE = `
<node>
  <interface name="org.amethystora.Agent">
    <method name="Ask">
      <arg type="s" direction="in" name="id"/>
      <arg type="s" direction="in" name="digest"/>
      <arg type="b" direction="out" name="allowed"/>
    </method>
    <method name="RoutinesChanged"/>
  </interface>
</node>`;
const ERROR = 'org.amethystora.Agent.Unavailable';
// The question went unanswered (the screen locked, the features went off, the hour ran out): the tool server says so, and does not take it for a no
const UNANSWERED = 'org.amethystora.Agent.Unanswered';
// Milliseconds a question's Allow ignores a click after the question appears, so that one meant for a button that was there before it is not taken for it
const SETTLE = 700;

Gio._promisify(Gio.Subprocess.prototype, 'communicate_utf8_async');
Gio._promisify(Gio.File.prototype, 'load_contents_async');
Gio._promisify(Gio.DBusConnection.prototype, 'call', 'call_finish');
Gio._promisify(Gio.File.prototype, 'query_info_async');
Gio._promisify(Gio.File.prototype, 'read_async');
Gio._promisify(Gio.File.prototype, 'delete_async');
Gio._promisify(Gio.InputStream.prototype, 'read_bytes_async');
Gio._promisify(Gio.InputStream.prototype, 'close_async');
// The private folder a picture's copy is made in, and taken from again when it has been asked about
const PRIVATE = GLib.build_filenamev([GLib.get_user_runtime_dir(), 'amethystora', 'ask']);

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
        this._unstage();
        this._subject = null;
        for (const question of [...this._questions])
            this._reply(question, null);
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

    // What is still asked is answered as not answered, not left on the card for a click nobody can send back
    unexport() {
        for (const question of [...this._questions])
            this._reply(question, null);
        this._dbus?.unexport();
        this._dbus = null;
    }

    // The key went off: what is asked is not asked here any more
    dropQuestions() {
        for (const question of [...this._questions])
            this._reply(question, null);
    }

    // Routines were kept, paused or taken away (`ame agent routines`): the desktop listens again
    RoutinesChanged() {
        this._host.routinesChanged?.();
    }

    // The process that made a bus call, and its parent: the tool server's own `gdbus`, which the server started
    async _callerParent(sender) {
        try {
            const reply = await Gio.DBus.session.call('org.freedesktop.DBus', '/org/freedesktop/DBus', 'org.freedesktop.DBus',
                'GetConnectionUnixProcessID', new GLib.Variant('(s)', [sender]), null, Gio.DBusCallFlags.NONE, 2000, null);
            const pid = reply.deepUnpack()[0];
            const [bytes] = await Gio.File.new_for_path(`/proc/${pid}/stat`).load_contents_async(null);
            const stat = new TextDecoder().decode(bytes);
            return Number(stat.slice(stat.lastIndexOf(')') + 2).split(' ')[1]);
        } catch {
            return 0;
        }
    }

    // Whether PID is one of this account's tool servers: its program the tools', `mcp` its only word, run by this account. The
    // server's number comes from a file the account can write, so it counts only for a process that really is one (changes.js
    // reads the sessions' files the same way)
    _isToolServer(pid) {
        try {
            if (!Number.isInteger(pid) || pid <= 1 || GLib.file_read_link(`/proc/${pid}/exe`) !== TOOLS)
                return false;
            const [, cmdline] = GLib.file_get_contents(`/proc/${pid}/cmdline`);
            if (new TextDecoder().decode(cmdline) !== `${TOOLS}\0mcp\0`)
                return false;
            const own = Gio.File.new_for_path('/proc/self').query_info('unix::uid', Gio.FileQueryInfoFlags.NONE, null).get_attribute_uint32('unix::uid');
            return Gio.File.new_for_path(`/proc/${pid}`).query_info('unix::uid', Gio.FileQueryInfoFlags.NONE, null).get_attribute_uint32('unix::uid') === own;
        } catch {
            return false;
        }
    }

    // The tool server asks, by the id of its hand-off: the question is in a file of the account's own runtime folder, written by
    // the server, so that the command a person reads is never made of a bus message. The reply is the person's click.
    async AskAsync([id, digest], invocation) {
        // Nobody can see a question without a live pill, and the key may be off: the terminal asks, as it always did
        if (!this._host.questionsOn || !isId(id) || typeof digest !== 'string' || !this._host.pill?.canAsk?.()) {
            invocation.return_dbus_error(ERROR, 'not asked here');
            return;
        }
        const file = Gio.File.new_for_path(GLib.build_filenamev([GLib.get_user_runtime_dir(), 'amethystora', 'agent-tools', id, 'ask.json']));
        let question;
        let bytes = null;
        try {
            [bytes] = await file.load_contents_async(null);
            question = parseQuestion(new TextDecoder().decode(bytes));
        } catch {
            question = null;
        }
        // What is read is what the tool server wrote (its digest comes in its own call's arguments), and the call is the server's
        // own child's: a file rewritten between the two, or a question from anywhere else, is not shown
        if (question && (GLib.compute_checksum_for_data(GLib.ChecksumType.SHA256, bytes) !== digest ||
            !question.server || !this._isToolServer(question.server) || await this._callerParent(invocation.get_sender()) !== question.server))
            question = null;
        // The extension may have stopped while the file was read
        if (!this._dbus || !this._host.questionsOn) {
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
            () => this._reply(question, null, true));
        question.timer = GLib.timeout_add_seconds(GLib.PRIORITY_DEFAULT, QUESTION_KEPT, () => {
            question.timer = 0;
            this._reply(question, null);
            return GLib.SOURCE_REMOVE;
        });
        question.since = GLib.get_monotonic_time();
        this._questions.push(question);
        this._sync();
    }

    // The person's answer, true or false, or none (null: it went unanswered), sent back, and the question off the card; `gone`: the
    // caller went away, so there is nobody to answer
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
                if (allowed === null)
                    question.invocation.return_dbus_error(UNANSWERED, 'nobody answered');
                else
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

    // --- Ask about this (R7-14) ----------------------------------------------------------------------------------------------

    // A file dropped on the pill: shown with where it goes, and sent only after Send and ask. One picture or text file at a time.
    // Nothing is copied, read past its first bytes, or sent before then; refused while the screen is shared.
    async aboutFiles(paths) {
        if (!this._host.on || this._process)
            return;
        if (this._host.pill?.sharing?.()) {
            this._host.pill.notice?.(_('Not while your screen is shared'), 'dialog-question-symbolic', 4);
            return;
        }
        const cancellable = new Gio.Cancellable();
        let picked;
        let file;
        try {
            file = Gio.File.new_for_path(paths[0] ?? '');
            const info = paths.length === 1
                ? await file.query_info_async('standard::type,standard::size,standard::name', Gio.FileQueryInfoFlags.NOFOLLOW_SYMLINKS, GLib.PRIORITY_DEFAULT, cancellable)
                : null;
            picked = info?.get_file_type() === Gio.FileType.REGULAR
                ? aboutFiles([{name: info.get_name(), size: info.get_size()}]) : aboutFiles(paths.length === 1 ? [{name: '', size: 0}] : paths);
        } catch {
            picked = {why: 'kind'};
        }
        if (!this._host.on)
            return;
        if (!picked.kind) {
            const why = {one: _('One file at a time'), big: _('That picture is too large to send'),
                kind: _('I can look at a picture or a text file')}[picked.why] ?? _('I can look at a picture or a text file');
            this._host.pill?.notice?.(why, 'dialog-question-symbolic', 4);
            return;
        }
        const subject = {kind: picked.kind, name: plain(picked.name, 60), size: picked.size, file};
        if (picked.kind === 'picture') {
            // What its header declares is judged before anything decodes it: a small file can declare a picture of gigabytes, and
            // the card draws it through GdkPixbuf. A header that cannot be read in the start of the file is refused too.
            let fits = false;
            try {
                const stream = await file.read_async(GLib.PRIORITY_DEFAULT, cancellable);
                const bytes = await stream.read_bytes_async(PICTURE_HEAD, GLib.PRIORITY_DEFAULT, cancellable);
                await stream.close_async(GLib.PRIORITY_DEFAULT, cancellable);
                fits = pictureFits(bytes.toArray());
            } catch {
                fits = false;
            }
            if (!this._host.on)
                return;
            if (!fits) {
                this._host.pill?.notice?.(_('That picture is too large to send'), 'dialog-question-symbolic', 4);
                return;
            }
        }
        if (picked.kind === 'text') {
            // The start of it, as text, read now to show what would go: nothing else of the file
            try {
                const stream = await file.read_async(GLib.PRIORITY_DEFAULT, cancellable);
                const bytes = await stream.read_bytes_async(TEXT_READ, GLib.PRIORITY_DEFAULT, cancellable);
                await stream.close_async(GLib.PRIORITY_DEFAULT, cancellable);
                subject.text = new TextDecoder().decode(bytes.toArray());
            } catch {
                this._host.pill?.notice?.(_('I can look at a picture or a text file'), 'dialog-question-symbolic', 4);
                return;
            }
        }
        this._confirm(subject);
    }

    // A picture on the clipboard (a screenshot of a region, copied), from the key when no text is selected
    aboutPicture(bytes) {
        if (!this._host.on || this._process)
            return;
        if (this._host.pill?.sharing?.()) {
            this._host.pill.notice?.(_('Not while your screen is shared'), 'dialog-question-symbolic', 4);
            return;
        }
        // Its header is judged before the card decodes it, as a dropped file's is
        if (bytes.get_size() > PICTURE_MAX || !pictureFits(bytes.toArray())) {
            this._host.pill?.notice?.(_('That picture is too large to send'), 'dialog-question-symbolic', 4);
            return;
        }
        this._confirm({kind: 'picture', name: _('The picture you copied'), size: bytes.get_size(), bytes});
    }

    _confirm(subject) {
        if (!this._host.pill?.canAsk?.()) {
            this._host.pill?.notice?.(_('The live pill is off, so there is nowhere to ask'), 'dialog-question-symbolic', 4);
            return;
        }
        this._unstage();
        this._subject = subject;
        this._mode = 'confirm';
        this._answer = null;
        this._sync();
        this._host.pill.openCard('ask');
    }

    // The picture's copy in the private folder, made now that the person said Send: a file of ours, 0600 in a 0700 folder, which is
    // taken away when the question is answered. Its path, or null
    async _stage(subject) {
        const name = `pic-${GLib.uuid_string_random().replace(/-/g, '')}.${pictureEnd(subject.file?.get_basename() ?? '')}`;
        GLib.mkdir_with_parents(PRIVATE, 0o700);
        const target = Gio.File.new_for_path(GLib.build_filenamev([PRIVATE, name]));
        try {
            const bytes = subject.bytes ?? (await subject.file.load_contents_async(null))[0];
            const data = bytes instanceof GLib.Bytes ? bytes : new GLib.Bytes(bytes);
            await new Promise((resolve, reject) => {
                target.replace_contents_bytes_async(data, null, false, Gio.FileCreateFlags.PRIVATE | Gio.FileCreateFlags.REPLACE_DESTINATION, null,
                    (file, result) => {
                        try {
                            resolve(file.replace_contents_finish(result));
                        } catch (e) {
                            reject(e);
                        }
                    });
            });
        } catch {
            return null;
        }
        this._staged = target;
        return target.get_path();
    }

    // The copy goes, whatever came of the question
    _unstage() {
        const target = this._staged;
        this._staged = null;
        target?.delete_async(GLib.PRIORITY_DEFAULT, null).catch(() => {});
    }

    async _sendAbout(text) {
        const subject = this._subject;
        if (!subject)
            return;
        // A share that began while the card was open: nothing is sent, and the picture is not left showing on it
        if (this._host.pill?.sharing?.()) {
            this._host.pill.notice?.(_('Not while your screen is shared'), 'dialog-question-symbolic', 4);
            this._leave();
            return;
        }
        const question = text.replace(/\s+/g, ' ').trim() || _('What is this, and is anything wrong?');
        let picture = '';
        if (subject.kind === 'picture') {
            this._mode = 'working';
            this._sync();
            picture = await this._stage(subject);
            if (!picture) {
                this._subject = null;
                this._mode = 'failed';
                this._answer = {kind: 'failed', why: 'error', detail: _('The picture could not be read.')};
                this._sync();
                return;
            }
        }
        const quoted = subject.text ?? '';
        this._subject = null;
        try {
            await this.ask(question, {quoted, picture});
        } finally {
            this._unstage();
        }
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
        this._unstage();
        this._subject = null;
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
        case 'confirm': return _('Ask about this');
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
                if (!this._body && (this._mode === 'typing' || this._mode === 'confirm'))
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
        if (this._entry && (this._mode === 'typing' || this._mode === 'confirm'))
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
            // A click right after the question came is one meant for what was under the pointer before it
            cardButton(_('Allow'), () => {
                if ((GLib.get_monotonic_time() - question.since) / 1000 >= SETTLE)
                    this._reply(question, true);
            }, {small: true, primary: true}),
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
        case 'confirm': {
            const subject = this._subject;
            box.add_child(this._head(_('Ask about this'), name));
            const row = new St.BoxLayout({style_class: 'amethystora-agent-act', x_expand: true});
            const icon = subject.kind === 'picture'
                ? new St.Icon({gicon: subject.bytes ? new Gio.BytesIcon({bytes: subject.bytes}) : new Gio.FileIcon({file: subject.file}), icon_size: 48})
                : new St.Icon({icon_name: 'text-x-generic-symbolic', icon_size: 24, y_align: Clutter.ActorAlign.CENTER});
            row.add_child(icon);
            const column = new St.BoxLayout({orientation: Clutter.Orientation.VERTICAL, x_expand: true, y_align: Clutter.ActorAlign.CENTER});
            column.add_child(new St.Label({style_class: 'amethystora-agent-act-what', text: subject.name}));
            column.add_child(new St.Label({style_class: 'amethystora-agent-act-how',
                text: subject.kind === 'picture' ? _('1 picture, %s').format(GLib.format_size(subject.size))
                    : _('A text file, %s').format(GLib.format_size(subject.size))}));
            row.add_child(column);
            box.add_child(row);
            const entry = new St.Entry({style_class: 'amethystora-agent-field', hint_text: _('What is this error?'), can_focus: true, x_expand: true});
            entry.clutter_text.max_length = 600;
            entry.text = this._typed;
            entry.clutter_text.connect('activate', () => this._sendAbout(entry.text));
            this._entry = entry;
            box.add_child(entry);
            // What goes, and to whom, before it goes
            box.add_child(this._text(subject.kind === 'picture'
                ? _('This picture goes to the maker of %s. What is on it goes with it.').format(name)
                : _('The start of this text goes to the maker of %s.').format(name), 'amethystora-agent-act-how'));
            buttons.push(cardButton(_('Cancel'), () => this._leave(), {small: true}),
                cardButton(_('Send and ask'), () => this._sendAbout(entry.text), {small: true, primary: true}));
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
