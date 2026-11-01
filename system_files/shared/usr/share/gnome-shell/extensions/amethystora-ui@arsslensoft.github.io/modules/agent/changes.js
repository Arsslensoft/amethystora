// The agent in the live pill: while the agent may make changes (level 2) and its tool server runs, each change its tools
// record shows in the pill as it happens, with its own Undo, and Stop ends the tool server so that it can change
// nothing more. The tools write every change, with its way back, to ~/.local/state/amethystora/agent-tools/actions.jsonl
// and name each running server in $XDG_RUNTIME_DIR/amethystora/agent-sessions; both are watched (two file monitors),
// and only what was appended is read. A change counts for this session when it was recorded after the first server
// that runs now started. When the last server ends, the pill says how many changes it made for DONE_FOR, then the clock
// is back. Undo runs `amethystora-agent undo ID` (the tools' own undo, with their checks); nothing here changes
// anything itself.

import Clutter from 'gi://Clutter';
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import Pango from 'gi://Pango';
import Shell from 'gi://Shell';
import St from 'gi://St';

import {gettext as _, ngettext} from 'resource:///org/gnome/shell/extensions/extension.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import * as Util from 'resource:///org/gnome/shell/misc/util.js';

import {buttonRow, cardButton, retire} from '../pill/clock.js';
import {plain, timeOfDay} from '../pill/text.js';
import {output} from './status.js';

const AGENT = 'amethystora-agent';
const TOOLS = '/usr/libexec/amethystora-agent-tools';
const IN_TERMINAL = '/usr/libexec/amethystora-in-terminal';
// Read of the record when the module starts: a session's changes are at its end
const TAIL = 262144;
// Seconds the pill says what a finished session changed
const DONE_FOR = 600;
// Changes listed in the card, the latest ones, oldest first
const ROWS = 4;

const decoder = new TextDecoder();
const CONTROL_LIST = '/usr/libexec/amethystora-control-list';
// The kernel's clock ticks a second, for a process's start time in /proc/PID/stat
const TICKS = 100;

Gio._promisify(Gio.File.prototype, 'query_info_async');
Gio._promisify(Gio.File.prototype, 'read_async');
Gio._promisify(Gio.InputStream.prototype, 'read_bytes_async');

// A small file the kernel makes up when asked (/proc), read at once: it never waits for a disk
function readNow(path) {
    try {
        return GLib.file_get_contents(path)[1];
    } catch {
        return null;
    }
}

// Whether a record's way back undoes exactly what it says it did (as the tools' own act::inverse): the person reads the
// forward fields, and Undo acts on the way back, so the two must agree before Undo is offered
function inverse(record) {
    const undo = record.undo;
    if (!undo || typeof undo !== 'object' || undo.tool !== record.tool)
        return false;
    if (record.tool === 'change')
        return undo.entry === record.entry && undo.value === record.before && typeof record.before === 'string';
    const back = {install: 'remove', remove: 'install'}[record.action];
    return record.tool === 'software' && !!back && undo.action === back && undo.source === record.source &&
        undo.id === record.name && (undo.container ?? null) === (record.container ?? null);
}

// A record's words as one line, each quoted when it needs to be (as the tools' own history does)
function words(argv) {
    if (!Array.isArray(argv))
        return '';
    return argv.filter(w => typeof w === 'string')
        .map(w => (/^[A-Za-z0-9_./:=@+,-]+$/.test(w) ? w : `'${w.replace(/'/g, "'\\''")}'`)).join(' ');
}

export class AgentChanges {
    // pill: the pill module (setAgent, agentChanged); agentTitle(): the chosen agent's name; gicon: the agent's mark
    constructor(pill, agentTitle, gicon) {
        this._pill = pill;
        this._agentTitle = agentTitle;
        this._gicon = gicon;
        this.id = 'agent';
        this.plate = 'agent';
    }

    start() {
        this._records = new Map();
        this._order = [];
        this._undoing = new Set();
        this._said = new Map();
        this._servers = new Map();
        this._sessionStart = 0;
        this._finished = 0;
        this._offset = 0;
        this._pending = new Uint8Array(0);
        this._titles = null;
        this._cancellable = new Gio.Cancellable();
        // Whose processes count as this account's, and when the machine started (a process's start is counted from it)
        this._uid = Gio.File.new_for_path('/proc/self').query_info('unix::uid', Gio.FileQueryInfoFlags.NONE, null)
            .get_attribute_uint32('unix::uid');
        this._boot = Number(/^btime (\d+)$/m.exec(decoder.decode(readNow('/proc/stat') ?? new Uint8Array(0)))?.[1] ?? 0);

        const state = GLib.build_filenamev([GLib.get_user_state_dir(), 'amethystora', 'agent-tools']);
        const sessions = GLib.build_filenamev([GLib.get_user_runtime_dir(), 'amethystora', 'agent-sessions']);
        // Made if missing, so that both are watched by inotify rather than looked for every few seconds
        GLib.mkdir_with_parents(state, 0o700);
        GLib.mkdir_with_parents(sessions, 0o700);
        this._log = Gio.File.new_for_path(GLib.build_filenamev([state, 'actions.jsonl']));
        this._sessions = Gio.File.new_for_path(sessions);
        try {
            this._logMonitor = this._log.monitor_file(Gio.FileMonitorFlags.NONE, this._cancellable);
            this._logMonitor.connect('changed', () => this._readLog());
            this._sessionMonitor = this._sessions.monitor_directory(Gio.FileMonitorFlags.NONE, this._cancellable);
            this._sessionMonitor.connect('changed', () => this._readSessions());
        } catch (e) {
            console.warn(`Amethystora UI: the agent's changes cannot be watched: ${e.message}`);
        }
        this._readLog();
        this._readSessions();
    }

    stop() {
        this._cancellable?.cancel();
        this._cancellable = null;
        this._logMonitor?.cancel();
        this._logMonitor = null;
        this._sessionMonitor?.cancel();
        this._sessionMonitor = null;
        if (this._doneTimer)
            GLib.source_remove(this._doneTimer);
        this._doneTimer = 0;
        if (this._recheck)
            GLib.source_remove(this._recheck);
        this._recheck = 0;
        this._show(false);
        this._records = null;
    }

    // --- Reading ------------------------------------------------------------------------------------------------

    // What was appended to the record since the last read (all of it from TAIL bytes before the end, the first time,
    // or when the file was replaced by a shorter one), off the shell's thread; then the servers are looked at again,
    // since a change is when one that died unseen matters
    async _readLog() {
        if (this._reading) {
            this._again = true;
            return;
        }
        this._reading = true;
        const cancellable = this._cancellable;
        try {
            do {
                this._again = false;
                // eslint-disable-next-line no-await-in-loop
                await this._readAppended(cancellable);
            } while (this._again && !cancellable.is_cancelled());
        } catch {}
        this._reading = false;
        if (this._records && !cancellable.is_cancelled())
            this._readSessions();
    }

    async _readAppended(cancellable) {
        const info = await this._log.query_info_async('standard::size', Gio.FileQueryInfoFlags.NONE, GLib.PRIORITY_DEFAULT, cancellable);
        const size = info.get_size();
        if (size < this._offset || this._offset === 0) {
            this._offset = Math.max(0, size - TAIL);
            this._pending = new Uint8Array(0);
        }
        if (size === this._offset || !this._records)
            return;
        const stream = await this._log.read_async(GLib.PRIORITY_DEFAULT, cancellable);
        stream.seek(this._offset, GLib.SeekType.SET, null);
        const bytes = await stream.read_bytes_async(size - this._offset, GLib.PRIORITY_DEFAULT, cancellable);
        stream.close(null);
        if (!this._records)
            return;
        this._offset += bytes.get_size();
        // Whole lines only, cut as bytes, so that a character a read cut in two waits for its other half
        const read = bytes.toArray();
        const all = new Uint8Array(this._pending.length + read.length);
        all.set(this._pending);
        all.set(read, this._pending.length);
        const end = all.lastIndexOf(10);
        this._pending = all.slice(end + 1);
        const lines = decoder.decode(all.subarray(0, end + 1)).split('\n');
        for (const line of lines) {
            let record;
            try {
                record = JSON.parse(line);
            } catch {
                continue;
            }
            if (typeof record?.id !== 'string' || !/^[0-9a-f]{8}$/.test(record.id))
                continue;
            if (!this._records.has(record.id))
                this._order.push(record.id);
            // A job writes its record again when it ends: the last one is how it stands
            this._records.set(record.id, record);
        }
        // Only what could still be shown is kept
        if (this._order.length > 400) {
            for (const id of this._order.splice(0, this._order.length - 400))
                this._records.delete(id);
        }
    }

    // When a tool server of this account started (seconds since the epoch), or 0 when the process is not one: its
    // program the tools', `mcp` its only word, run by this account; the start from the kernel, never from a file
    _server(pid) {
        try {
            if (GLib.file_read_link(`/proc/${pid}/exe`) !== TOOLS)
                return 0;
            if (decoder.decode(readNow(`/proc/${pid}/cmdline`) ?? new Uint8Array(0)) !== `${TOOLS}\0mcp\0`)
                return 0;
            if (Gio.File.new_for_path(`/proc/${pid}`).query_info('unix::uid', Gio.FileQueryInfoFlags.NONE, null)
                .get_attribute_uint32('unix::uid') !== this._uid)
                return 0;
            const stat = decoder.decode(readNow(`/proc/${pid}/stat`));
            return this._boot + Math.floor(Number(stat.slice(stat.lastIndexOf(')') + 2).split(' ')[19]) / TICKS);
        } catch {
            return 0;
        }
    }

    // The tool servers running now: each named by a file in the runtime folder (an in-memory folder, read at once), and
    // still the live process it names. A server that ended by a signal it could not answer leaves its name behind; it
    // is found gone here, which runs at every change of the folder or the record and when the card opens
    _readSessions() {
        const servers = new Map();
        try {
            const children = this._sessions.enumerate_children('standard::name', Gio.FileQueryInfoFlags.NONE, null);
            for (let info = children.next_file(null); info; info = children.next_file(null)) {
                const pid = info.get_name();
                const started = /^[0-9]+$/.test(pid) ? this._server(pid) : 0;
                if (started)
                    servers.set(pid, started);
            }
            children.close(null);
        } catch {}
        const before = this._servers.size;
        this._servers = servers;
        if (servers.size > 0) {
            const start = Math.min(...servers.values());
            if (before === 0 || start < this._sessionStart || this._finished)
                this._sessionStart = start;
            this._finished = 0;
            if (this._doneTimer)
                GLib.source_remove(this._doneTimer);
            this._doneTimer = 0;
        } else if (before > 0) {
            this._finish();
        }
        this._sync();
    }

    // The last server ended: the pill says what the session did for DONE_FOR, if it did anything
    _finish() {
        this._finished = Math.floor(Date.now() / 1000);
        if (this._doneTimer)
            GLib.source_remove(this._doneTimer);
        this._doneTimer = GLib.timeout_add_seconds(GLib.PRIORITY_DEFAULT, DONE_FOR, () => {
            this._doneTimer = 0;
            this._sessionStart = 0;
            this._finished = 0;
            this._sync();
            return GLib.SOURCE_REMOVE;
        });
    }

    // --- What the session did -----------------------------------------------------------------------------------

    // This session's changes, as their last records say, oldest first: not the undoing of another, and not reads
    get changes() {
        if (!this._sessionStart || !this._records)
            return [];
        return this._order.map(id => this._records.get(id))
            .filter(r => r && !r.of && ['change', 'software', 'run'].includes(r.tool) && Number(r.time) >= this._sessionStart);
    }

    // What the session really changed: not what was declined, refused or failed, or what never finished
    get made() {
        return this.changes.filter(r => r.result === 'ok');
    }

    // Whether a change was undone: a finished record of its undoing that did its way back (as the tools' act::undoes)
    _undone(record) {
        const undo = record.undo;
        if (!undo)
            return false;
        for (const other of this._records.values()) {
            if (other.of === record.id && other.result === 'ok' && other.tool === undo.tool && (undo.tool === 'change'
                ? other.entry === undo.entry && other.value === undo.value
                : other.action === undo.action && other.name === undo.id))
                return true;
        }
        return false;
    }

    // Whether the agent waits for the person: a command or a change handed to a terminal, or a root command in the mode
    // that asks for a password every time, not yet answered
    get asking() {
        // Only a server still running can be waiting: one that ended left its records as they were
        return [...this.changes].reverse().find(r => this._servers.has(String(r.server)) && (r.result === 'opened' ||
            (r.tool === 'run' && r.root === true && r.result === 'running' && r.mode === 'each'))) ?? null;
    }

    // Control's own titles of its settings and values, for the changes' rows: asked once, when the agent first changes
    // something; until they come, the setting's id and value show. The row says what the record's way back undoes
    async _loadTitles() {
        this._titles = new Map();
        const listed = await output([CONTROL_LIST, '--json']);
        if (!this._records)
            return;
        try {
            for (const item of JSON.parse(listed.text).items ?? [])
                this._titles.set(item.id, {title: item.title, values: new Map((item.choices ?? []).map(c => [c.value, c.title]))});
        } catch {}
        this._fillCard();
    }

    get state() {
        if (this.changes.length === 0)
            return null;
        if (this._servers.size === 0)
            return this._finished && this.made.length > 0 ? 'done' : null;
        return this.asking ? 'ask' : 'work';
    }

    // --- In the pill --------------------------------------------------------------------------------------------

    _sync() {
        if (!this._records)
            return;
        const state = this.state;
        this.plate = {ask: 'agent-ask', done: 'agent-done'}[state] ?? 'agent';
        this._show(state !== null);
        if (state !== null) {
            if (!this._titles)
                this._loadTitles();
            if (this._label)
                this._label.text = this.text;
            this._pill?.agentChanged?.();
            this._fillCard();
        }
    }

    _show(shown) {
        if (shown === !!this._shown)
            return;
        this._shown = shown;
        this._pill?.setAgent?.(shown ? this : null);
        if (!shown) {
            // Out of the pill first (its fade may still hold it), then gone: nothing is left for the pill to let go of
            if (this._content)
                this._pill?.clock?.forget(this._content);
            retire(this._content);
            this._content = null;
            this._label = null;
            this._cardBody = null;
        }
    }

    get text() {
        const state = this.state;
        if (state === 'done') {
            const count = this.made.length;
            return ngettext('Agent made %d change', 'Agent made %d changes', count).format(count);
        }
        if (state === 'ask')
            return this.asking?.root ? _('Agent asks for root') : _('Agent asks you');
        const last = this.changes.at(-1);
        if (last?.tool === 'software' && last.result === 'running')
            return last.action === 'remove' ? _('Agent · removing an app') : _('Agent · installing an app');
        if (last?.tool === 'change')
            return _('Agent · changing settings');
        return _('Agent · making changes');
    }

    get accessible() {
        return this.text;
    }

    get row() {
        return {icon: this._gicon, text: this.text};
    }

    pill() {
        if (!this._content) {
            this._content = new St.BoxLayout({style_class: 'amethystora-pill-content amethystora-agent-pill'});
            this._content.add_child(new St.Icon({gicon: this._gicon, icon_size: 13, y_align: Clutter.ActorAlign.CENTER}));
            this._label = new St.Label({text: this.text, y_align: Clutter.ActorAlign.CENTER});
            this._label.clutter_text.use_markup = false;
            this._content.add_child(this._label);
        }
        return this._content;
    }

    card(api) {
        this._api = api;
        const body = new St.BoxLayout({style_class: 'amethystora-pill-card-body amethystora-agent-card', orientation: Clutter.Orientation.VERTICAL});
        this._cardBody = body;
        body.connect('destroy', () => {
            if (this._cardBody === body)
                this._cardBody = null;
        });
        this._fillCard();
        // A server that died unseen is found gone when the card opens (after the card is made, not while)
        this._recheck ||= GLib.idle_add(GLib.PRIORITY_DEFAULT, () => {
            this._recheck = 0;
            this._readSessions();
            return GLib.SOURCE_REMOVE;
        });
        return body;
    }

    // The card's contents, again whenever the session changes while it is open
    _fillCard() {
        const body = this._cardBody;
        if (!body)
            return;
        body.destroy_all_children();
        const state = this.state;
        const head = new St.BoxLayout({style_class: 'amethystora-agent-card-head'});
        const title = state === 'done'
            ? _('Finished at %s').format(timeOfDay(new Date(this._finished * 1000)))
            : state === 'ask' ? _('Waiting for you') : _('Changes so far');
        head.add_child(new St.Label({style_class: 'amethystora-agent-card-title', text: title, x_expand: true, y_align: Clutter.ActorAlign.END}));
        head.add_child(new St.Label({style_class: 'amethystora-agent-card-who', text: this._agentTitle(), y_align: Clutter.ActorAlign.END}));
        body.add_child(head);

        const asking = this.asking;
        for (const record of this.changes.filter(r => r !== asking).slice(-ROWS))
            body.add_child(this._row(record));
        if (asking)
            body.add_child(this._askingRow(asking));

        const buttons = [];
        const undoable = this.changes.filter(r => this._canUndo(r));
        if (undoable.length > 1)
            buttons.push(cardButton(_('Undo all'), () => this._undoAll(undoable), {small: true}));
        if (this._servers.size > 0)
            buttons.push(cardButton(_('Stop the agent'), () => this._stop(), {small: true}));
        const row = buttonRow(buttons);
        row.add_style_class_name('amethystora-agent-card-buttons');
        row.x_expand = true;
        row.x_align = Clutter.ActorAlign.FILL;
        const history = new St.Button({style_class: 'amethystora-agent-link', label: _('History'), can_focus: true, x_expand: true,
            x_align: Clutter.ActorAlign.END, y_align: Clutter.ActorAlign.CENTER});
        history.connect('clicked', () => {
            this._api?.close();
            Util.spawn([IN_TERMINAL, AGENT, 'history']);
        });
        row.add_child(history);
        body.add_child(row);
    }

    _canUndo(record) {
        return record.result === 'ok' && inverse(record) && !this._undone(record) && !this._undoing.has(record.id);
    }

    // One change: what it was, how and when, and its Undo (or how it stands)
    _row(record) {
        const row = new St.BoxLayout({style_class: 'amethystora-agent-act', x_expand: true});
        const column = new St.BoxLayout({orientation: Clutter.Orientation.VERTICAL, x_expand: true});
        const what = new St.Label({style_class: 'amethystora-agent-act-what', text: this._what(record)});
        const how = new St.Label({style_class: 'amethystora-agent-act-how', text: this._said.get(record.id) ?? this._how(record)});
        for (const label of [what, how]) {
            label.clutter_text.use_markup = false;
            label.clutter_text.ellipsize = Pango.EllipsizeMode.END;
        }
        column.add_child(what);
        column.add_child(how);
        row.add_child(column);

        let end = null;
        if (this._canUndo(record)) {
            end = new St.Button({style_class: 'amethystora-agent-link', label: _('Undo'), can_focus: true, y_align: Clutter.ActorAlign.CENTER});
            end.accessible_name = _('Undo: %s').format(this._what(record));
            end.connect('clicked', () => this._undo(record.id));
        } else {
            // An undo handed to a terminal waits there for the person; a change whose server is gone is not finished
            const waiting = [...this._records.values()].some(r => r.of === record.id && r.result === 'opened');
            const word = this._undoing.has(record.id) ? (waiting ? _('Waiting in its terminal') : _('Undoing…'))
                : this._undone(record) ? _('Undone')
                    : record.result === 'running' && this._servers.size === 0 ? _('Not finished')
                        : {failed: _('Failed'), declined: _('Declined'), refused: _('Refused'), 'not run': _('Not run'), running: _('Running…')}[record.result] ?? '';
            if (word)
                end = new St.Label({style_class: 'amethystora-agent-act-state', text: word, y_align: Clutter.ActorAlign.CENTER});
        }
        if (end)
            row.add_child(end);
        return row;
    }

    // What waits for the person: a root command, a command or a change handed to a terminal; Show brings the terminal
    _askingRow(record) {
        const row = new St.BoxLayout({style_class: 'amethystora-agent-act amethystora-agent-act-ask', x_expand: true});
        const column = new St.BoxLayout({orientation: Clutter.Orientation.VERTICAL, x_expand: true});
        const what = record.tool === 'run'
            ? (record.root ? _('Wants to run as root') : _('Wants to run a command'))
            : _('Wants to change a setting');
        const where = record.result === 'opened'
            ? (record.root ? _('asks for your password in its terminal') : _('asks you in its terminal'))
            : _('asks for your password');
        const detail = record.tool === 'run' ? words(record.argv) : plain(this._titles?.get(record.entry)?.title ?? record.entry ?? '', 80);
        const labels = [new St.Label({style_class: 'amethystora-agent-act-what', text: what}),
            new St.Label({style_class: 'amethystora-agent-act-how', text: `${plain(detail, 120)} · ${where}`})];
        for (const label of labels) {
            label.clutter_text.use_markup = false;
            label.clutter_text.ellipsize = Pango.EllipsizeMode.END;
            column.add_child(label);
        }
        row.add_child(column);
        const terminal = record.result === 'opened' ? this._terminal() : null;
        if (terminal) {
            const show = new St.Button({style_class: 'amethystora-agent-link', label: _('Show'), can_focus: true, y_align: Clutter.ActorAlign.CENTER});
            show.connect('clicked', () => {
                this._api?.close();
                Main.activateWindow(terminal);
            });
            row.add_child(show);
        }
        return row;
    }

    // The latest window of a terminal app, where a hand-off opens
    _terminal() {
        let best = null;
        for (const app of Shell.AppSystem.get_default().get_running()) {
            if (!app.get_app_info()?.get_categories()?.split(';').includes('TerminalEmulator'))
                continue;
            for (const window of app.get_windows()) {
                if (!best || window.get_user_time() > best.get_user_time())
                    best = window;
            }
        }
        return best;
    }

    _what(record) {
        switch (record.tool) {
        case 'change': {
            // The setting and value the way back is checked against (inverse), in Control's own words
            const item = this._titles?.get(record.entry);
            return `${plain(item?.title ?? record.entry ?? '', 60)}: ${plain(item?.values.get(record.value) ?? record.value ?? '', 60)}`;
        }
        case 'software': {
            const name = plain(record.name ?? '', 60);
            const running = record.result === 'running';
            if (record.action === 'remove')
                return (running ? _('Removing %s') : _('Removed %s')).format(name);
            return (running ? _('Installing %s') : _('Installed %s')).format(name);
        }
        case 'run':
            return record.root ? _('Ran a command as root') : _('Ran a command');
        default:
            return plain(record.tool ?? '', 40);
        }
    }

    _how(record) {
        let how;
        if (record.tool === 'software') {
            const argv = Array.isArray(record.argv) ? record.argv : [];
            how = record.source === 'flathub'
                ? (argv.includes('--user') ? _('Flatpak from Flathub, for you only') : _('Flatpak from Flathub, for everyone'))
                : record.source === 'brew' ? _('Homebrew') : _('In the container %s').format(plain(record.container ?? '', 40));
        } else {
            how = words(record.argv);
        }
        const time = Number(record.time) ? timeOfDay(new Date(Number(record.time) * 1000)) : '';
        return plain(time ? `${how} · ${time}` : how, 140);
    }

    // --- What the person does -----------------------------------------------------------------------------------

    async _undo(id) {
        if (this._undoing.has(id))
            return;
        this._undoing.add(id);
        this._said.delete(id);
        this._fillCard();
        const result = await output([AGENT, 'undo', id]);
        if (!this._records)
            return;
        this._undoing.delete(id);
        if (!result.ok) {
            // The tools' own words for why not, on the row
            const why = plain(result.text.trim().split('\n').at(-1) ?? '', 140);
            this._said.set(id, why || _('It could not be undone.'));
        }
        this._fillCard();
    }

    // Newest first, one after the other, so that a later change is taken back before the one it may build on
    async _undoAll(records) {
        for (const record of [...records].reverse()) {
            if (!this._records)
                return;
            // eslint-disable-next-line no-await-in-loop
            await this._undo(record.id);
        }
    }

    _stop() {
        Util.spawn([AGENT, 'stop']);
    }

    // For the suites: how it stands
    get status() {
        return {
            shown: !!this._shown, state: this.state, text: this._shown ? this.text : '', servers: this._servers?.size ?? 0,
            changes: this.changes.map(r => ({id: r.id, what: this._what(r), undo: this._canUndo(r), undone: this._undone(r),
                said: this._said.get(r.id) ?? null})),
            asking: this.asking?.id ?? null, show: this.asking?.result === 'opened' && !!this._terminal(), plate: this.plate,
        };
    }
}
