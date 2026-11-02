// Printing that talks: while a job of this account is in the queue, the pill says "Printing", the real pages when the
// printer gives them, and what is wrong in words (out of paper, a stuck sheet, the printer gone away), with Cancel, and
// Try again when there is something to try. A printer that says nothing gets "Printing" and no more.
//
// CUPS tells the system bus of a job without being asked: com.redhat.PrinterSpooler's JobQueuedLocal and JobStartedLocal
// (printer, job, owner) and QueueChanged (printer), silent unless somebody prints. Only root's are believed: the bus lets
// any account send them, so the sender's user is asked once (GetConnectionUnixUser) before a signal counts. So one
// subscription is all this costs until there is a job. CUPS has no per-page signal that is safe to listen to (the
// notifier's carry the document's name to the whole bus), so while a job moves ipptool asks for its state every few
// seconds, one ipptool for all of a printer's jobs, and less often once nothing has changed for a minute. After ten
// minutes unchanged the polling stops: the pill steps back behind music and updates (it stays in the card), QueueChanged
// and the card's opening ask once more, and an hour's timer is the last resort. No file is written, and the document's name
// is never asked for: jobs-state.test lists what is, and the other accounts' jobs are left alone.
//
// The key printing of org.gnome.shell.extensions.amethystora-ui.pill (ame desktop printing, Control), read live.

import Clutter from 'gi://Clutter';
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import St from 'gi://St';

import {gettext as _, ngettext} from 'resource:///org/gnome/shell/extensions/extension.js';
import * as Util from 'resource:///org/gnome/shell/misc/util.js';

import {buttonRow, cardButton, retire} from './clock.js';
import {OVER, admit, askArgv, byPrinter, parseState, parseStates, retryCommand, trouble} from './printwords.js';

Gio._promisify(Gio.DBusConnection.prototype, 'call', 'call_finish');

const SPOOLER = 'com.redhat.PrinterSpooler';
const SPOOLER_PATH = '/com/redhat/PrinterSpooler';
const IPPTOOL = '/usr/bin/ipptool';
const CANCEL = '/usr/bin/cancel';
const QUESTION = '/usr/share/amethystora/print/jobs-state.test';
// How soon a change is read (ms), the least between two asks that a queue change makes (ms), how often while the job moves
// (s), once nothing has changed in QUIET_AFTER_S, and once nothing has changed in STUCK_AFTER_S (the last resort), and
// how long the pill's last word stays (s)
const SETTLE_MS = 150;
const MIN_GAP_MS = 3000;
const OFTEN_S = 3;
const SELDOM_S = 30;
const FALLBACK_S = 3600;
const QUIET_AFTER_S = 60;
const STUCK_AFTER_S = 600;
const LAST_WORD_S = 4;
const ASK_LIMIT_S = 20;
// The pill's two places, by activity id: in front while the job moves, behind the music and the updates once it has been
// stuck for ten minutes
const FRONT = 'printing';
const BACK = 'printing-stuck';

// The jobs being followed, by number, each with its printer, and the last that was known of each. In memory only,
// outside the class, since a screen lock stops the extension and the job goes on printing.
const JOBS = new Map();
const LAST = new Map();
// What the last round saw, as text, and since when it has been the same (monotonic ms)
const FOLLOW = {signature: null, changedAt: 0};
// The user of each unique name that sent a signal: asked once. Anyone may send on the system bus, only root is believed.
const SENDERS = new Map();
const FEW_SENDERS = 16;

const now = () => GLib.get_monotonic_time() / 1000;

// Whether the sender of a signal is root (cupsd), by the bus's own word
function fromRoot(sender) {
    if (!SENDERS.has(sender)) {
        if (SENDERS.size >= FEW_SENDERS)
            SENDERS.clear();
        SENDERS.set(sender, Gio.DBus.system.call('org.freedesktop.DBus', '/org/freedesktop/DBus', 'org.freedesktop.DBus',
            'GetConnectionUnixUser', new GLib.Variant('(s)', [sender]), new GLib.VariantType('(u)'),
            Gio.DBusCallFlags.NONE, -1, null).then(reply => reply.deepUnpack()[0] === 0).catch(() => {
            SENDERS.delete(sender);
            return false;
        }));
    }
    return SENDERS.get(sender);
}

export class Printing {
    constructor(activities, notice) {
        this.id = FRONT;
        this.plate = '';
        this._activities = activities;
        this._notice = notice;
        this._view = null;
    }

    enable() {
        this._user = GLib.get_user_name();
        this._cancellable = new Gio.Cancellable();
        this._subscription = Gio.DBus.system.signal_subscribe(null, SPOOLER, null, SPOOLER_PATH, null,
            Gio.DBusSignalFlags.NONE, (c, sender, p, i, name, parameters) => this._signal(sender, name, parameters));
        if (JOBS.size > 0) {
            // What ended while this was off or the screen locked is not announced as news
            this._silent = true;
            this._watch();
        }
    }

    disable() {
        if (this._subscription)
            Gio.DBus.system.signal_unsubscribe(this._subscription);
        this._subscription = 0;
        this._cancellable?.cancel();
        this._cancellable = null;
        this._stopTimers();
        this._end();
    }

    _stopTimers() {
        if (this._settle)
            GLib.source_remove(this._settle);
        this._settle = 0;
        if (this._next)
            GLib.source_remove(this._next);
        this._next = 0;
    }

    _signal(sender, name, parameters) {
        if (!sender)
            return;
        fromRoot(sender).then(root => {
            if (root && this._cancellable)
                this._accept(name, parameters);
        });
    }

    _accept(name, parameters) {
        if (name === 'QueueChanged') {
            if (JOBS.size > 0)
                this._askSoon(MIN_GAP_MS);
            return;
        }
        if ((name !== 'JobQueuedLocal' && name !== 'JobStartedLocal') || parameters.get_type_string() !== '(sus)')
            return;
        const [printer, id, owner] = parameters.deepUnpack();
        // More than a person prints at once is not followed: the queue is for the printer to work through
        if (!admit(JOBS, printer, id, owner, this._user))
            return;
        FOLLOW.changedAt = now();
        this._watch();
    }

    // The pill says Printing at once, and the first answer comes a moment later
    _watch() {
        this._stuck = this._isStuck();
        this._show();
        this._askSoon();
    }

    _isStuck() {
        return FOLLOW.signature !== null && now() - FOLLOW.changedAt >= STUCK_AFTER_S * 1000;
    }

    // In a moment; `gap` is the least since the last ask, for what a signal from CUPS says
    _askSoon(gap = 0) {
        if (this._settle)
            return;
        const wait = gap > 0 && this._lastAsk ? Math.max(SETTLE_MS, this._lastAsk + gap - now()) : SETTLE_MS;
        this._settle = GLib.timeout_add(GLib.PRIORITY_DEFAULT, Math.ceil(wait), () => {
            this._settle = 0;
            this._askAll();
            return GLib.SOURCE_REMOVE;
        });
    }

    // What ipptool printed for these jobs of one printer, or null when it could not ask (not running, no cupsd, too slow)
    _ask(printer, ids) {
        return new Promise(resolve => {
            let process;
            try {
                process = Gio.Subprocess.new(askArgv(IPPTOOL, QUESTION, this._user, printer, ids),
                    Gio.SubprocessFlags.STDOUT_PIPE | Gio.SubprocessFlags.STDERR_SILENCE);
            } catch (e) {
                console.warn(`Amethystora UI: ipptool did not start: ${e.message}`);
                resolve(null);
                return;
            }
            // ipptool has its own time limit; this is for one that does not keep it
            let guard = GLib.timeout_add_seconds(GLib.PRIORITY_DEFAULT, ASK_LIMIT_S, () => {
                guard = 0;
                process.force_exit();
                return GLib.SOURCE_REMOVE;
            });
            process.communicate_utf8_async(null, this._cancellable, (p, result) => {
                if (guard)
                    GLib.source_remove(guard);
                guard = 0;
                try {
                    const [, stdout] = p.communicate_utf8_finish(result);
                    // Nothing on standard output is no answer (cupsd not there); CUPS saying no such job is one
                    resolve(stdout?.trim() ? stdout : null);
                } catch {
                    resolve(null);
                }
            });
        });
    }

    // The views of these jobs of one printer, by number (a job CUPS no longer has is not in it), or null when it could not
    // ask. One request for them all; if CUPS has dropped one of them from its history it refuses the whole request, and
    // they are asked one by one.
    async _askPrinter(printer, ids) {
        const text = await this._ask(printer, ids);
        if (text === null)
            return null;
        const views = parseStates(text);
        if (views.size > 0 || ids.length < 2)
            return views;
        const each = new Map();
        for (const id of ids) {
            // eslint-disable-next-line no-await-in-loop
            const one = await this._ask(printer, [id]);
            if (one === null)
                return null;
            const view = parseStates(one).get(id);
            if (view)
                each.set(id, view);
        }
        return each;
    }

    // One round at a time; a change during a round asks again after it
    async _askAll() {
        if (this._asking) {
            this._again = true;
            return;
        }
        this._asking = true;
        this._lastAsk = now();
        const cancellable = this._cancellable;
        const seen = [];
        const ended = [];
        for (const [printer, ids] of byPrinter(JOBS)) {
            // eslint-disable-next-line no-await-in-loop
            const views = await this._askPrinter(printer, ids);
            if (cancellable?.is_cancelled() || cancellable !== this._cancellable) {
                this._asking = false;
                return;
            }
            for (const id of ids) {
                // Not asked at all: the job is as it was, not gone
                const view = views ? views.get(id) ?? null : LAST.get(id) ?? undefined;
                if (view === undefined)
                    continue;
                if (!view || OVER.has(view.state)) {
                    JOBS.delete(id);
                    LAST.delete(id);
                    if (view)
                        ended.push(view.state);
                } else {
                    LAST.set(id, view);
                    seen.push({id, printer, view});
                }
            }
        }
        this._asking = false;
        this._apply(seen, ended);
        if (this._again) {
            this._again = false;
            this._askSoon();
        }
    }

    _apply(seen, ended) {
        const silent = this._silent;
        this._silent = false;
        const cancelling = this._cancelling;
        const failed = ended.includes('aborted');
        if (JOBS.size === 0) {
            this._cancelling = false;
            this._stopTimers();
            this._end();
            if (!cancelling && !silent && failed)
                this._notice(_('The printer could not finish this'), 'printer-error-symbolic', LAST_WORD_S);
            else if (!cancelling && !silent && ended.includes('completed'))
                this._notice(_('Printed'), 'printer-symbolic', LAST_WORD_S);
            return;
        }
        // One of several failed: it is said now, not when the last one is over
        if (failed && !silent && !cancelling)
            this._notice(_('The printer could not finish this'), 'printer-error-symbolic', LAST_WORD_S);
        if (seen.length === 0) {
            // Nothing could be asked: the pill says what it said, and the next ask comes in its time
            this._rearm();
            return;
        }
        // The job the printer is on, or else the first in the queue: what is wrong with it is what the pill says
        const first = seen.find(job => job.view.state === 'processing') ?? seen[0];
        const signature = JSON.stringify(seen);
        if (signature !== FOLLOW.signature) {
            FOLLOW.signature = signature;
            FOLLOW.changedAt = now();
        }
        this._first = first;
        this._waiting = seen.length - 1;
        this._view = first.view;
        this._stuck = this._isStuck();
        this._show();
        this._refresh();
        this._rearm();
    }

    // The next ask: soon while it moves, seldom once it has long stood still, and after ten minutes only the last resort
    _rearm() {
        if (this._next)
            GLib.source_remove(this._next);
        const standing = (now() - FOLLOW.changedAt) / 1000;
        let after = OFTEN_S;
        if (standing >= STUCK_AFTER_S)
            after = FALLBACK_S;
        else if (standing >= QUIET_AFTER_S)
            after = SELDOM_S;
        this._next = GLib.timeout_add_seconds(GLib.PRIORITY_DEFAULT, after, () => {
            this._next = 0;
            this._askAll();
            return GLib.SOURCE_REMOVE;
        });
    }

    _show() {
        this.pill();
        this._place();
    }

    // In front while the job moves, behind once it has stood still for ten minutes (still in the card, and the pill when
    // nothing else is going on)
    _place() {
        const id = this._stuck ? BACK : FRONT;
        if (this.id !== id) {
            if (this._activities.get(this.id) === this)
                this._activities.set(this.id, null);
            this.id = id;
        }
        if (this._activities.get(id) !== this)
            this._activities.set(id, this);
    }

    _end() {
        this.plate = '';
        this._view = null;
        this._first = null;
        this._waiting = 0;
        this._stuck = false;
        FOLLOW.signature = null;
        for (const id of [FRONT, BACK]) {
            if (this._activities.get(id) === this)
                this._activities.set(id, null);
        }
        this.id = FRONT;
        retire(this._content);
        this._content = null;
        this._label = null;
        this._icon = null;
    }

    get shown() {
        return this._activities.get(this.id) === this;
    }

    // --- What it says -----------------------------------------------------------------------------------------------

    // [title, sentence] for what is wrong, or null. Each a whole sentence, for the translators.
    _words() {
        const page = this._view?.done ? this._view.done + 1 : 0;
        switch (trouble(this._view)) {
        case 'paper':
            return [_('The printer is out of paper'), page
                ? _('It will carry on from page %d when you add more.').format(page)
                : _('It will carry on when you add more.')];
        case 'jam':
            return [_('The paper is stuck'), _('Take out the stuck sheet, and it will carry on.')];
        case 'ink':
            return [_('The printer is out of ink or toner'), _('It will carry on once it has more.')];
        case 'cover':
            return [_('A cover on the printer is open'), _('It will carry on once it is closed.')];
        case 'offline':
            return [_('The printer is not answering'), _('Check that it is on and connected. Your pages wait here until it answers.')];
        case 'stopped':
            return [_('The printer has stopped'), _('Your pages wait here. Try again starts it, or Cancel clears them.')];
        case 'job-stopped':
            return [_('This print job has stopped'), _('Try again carries on with it, or Cancel clears it.')];
        case 'held':
            return [_('This print job is on hold'), _('Try again sends it to the printer, or Cancel clears it.')];
        default:
            return null;
        }
    }

    // A few words for the pill while something is wrong
    _shortWords() {
        switch (trouble(this._view)) {
        case 'paper':
            return _('Out of paper');
        case 'jam':
            return _('Paper stuck');
        case 'ink':
            return _('Out of ink or toner');
        case 'cover':
            return _('Printer cover open');
        case 'offline':
            return _('Printer not answering');
        case 'stopped':
        case 'job-stopped':
            return _('Printing stopped');
        case 'held':
            return _('Printing on hold');
        default:
            return null;
        }
    }

    get text() {
        const short = this._shortWords();
        if (short)
            return short;
        const {done, total, state} = this._view ?? {};
        if (state === 'pending' && !done)
            return _('Waiting to print');
        if (done && total) {
            return ngettext('Printing · %d of %d page', 'Printing · %d of %d pages', total)
                .format(done, total);
        }
        if (done)
            return _('Printing · page %d').format(done);
        return _('Printing');
    }

    get accessible() {
        return this.text;
    }

    get row() {
        return {icon: trouble(this._view) ? 'printer-error-symbolic' : 'printer-symbolic', text: this.text};
    }

    pill() {
        if (!this._content) {
            this._content = new St.BoxLayout({style_class: 'amethystora-pill-content amethystora-pill-printing'});
            this._icon = new St.Icon({icon_name: this.row.icon, icon_size: 14, y_align: Clutter.ActorAlign.CENTER});
            this._label = new St.Label({text: this.text, y_align: Clutter.ActorAlign.CENTER});
            this._content.add_child(this._icon);
            this._content.add_child(this._label);
        }
        return this._content;
    }

    // Only what changed, so that nothing wakes while the words stay the same
    _refresh() {
        const text = this.text;
        const plate = trouble(this._view) && trouble(this._view) !== 'held' ? 'battery' : '';
        const changed = this._label?.text !== text || this.plate !== plate;
        if (this._label && this._label.text !== text)
            this._label.text = text;
        if (this._icon && this._icon.icon_name !== this.row.icon)
            this._icon.icon_name = this.row.icon;
        this.plate = plate;
        if (changed)
            this._activities.update(this.id);
        this._fill();
    }

    _fill() {
        if (!this._cardTitle)
            return;
        const words = this._words();
        this._cardTitle.text = words?.[0] ?? this.text;
        const lines = [];
        if (words)
            lines.push(words[1]);
        if (this._waiting > 0) {
            lines.push(ngettext('%d more waiting after this.', '%d more waiting after this.', this._waiting)
                .format(this._waiting));
        }
        this._cardText.text = lines.join(' ');
        this._cardText.visible = lines.length > 0;
        if (this._retryButton)
            this._retryButton.visible = !!this._retry();
        if (this._cancelButton)
            this._cancelButton.label = this._waiting > 0 ? _('Cancel all') : _('Cancel');
    }

    _retry() {
        const first = this._first;
        return first ? retryCommand(first.view, first.id, first.printer) : null;
    }

    card(api) {
        const body = new St.BoxLayout({style_class: 'amethystora-pill-card-body', orientation: Clutter.Orientation.VERTICAL});
        this._cardTitle = new St.Label({style_class: 'amethystora-pill-card-label'});
        this._cardText = new St.Label({style_class: 'amethystora-pill-card-text'});
        this._cardText.clutter_text.line_wrap = true;
        body.add_child(this._cardTitle);
        body.add_child(this._cardText);
        this._cancelButton = cardButton(_('Cancel'), () => {
            api.close();
            this.cancel();
        });
        this._retryButton = cardButton(_('Try again'), () => {
            api.close();
            this.retry();
        }, {primary: true});
        body.add_child(buttonRow([this._cancelButton, this._retryButton]));
        // The card goes when it closes: these are not to be touched after that
        body.connect('destroy', () => {
            this._cardTitle = null;
            this._cardText = null;
            this._cancelButton = null;
            this._retryButton = null;
        });
        this._fill();
        // A job that stood still for long is read once when its card opens, whatever CUPS has said
        this._askSoon(MIN_GAP_MS);
        return body;
    }

    // --- Cancel and Try again ---------------------------------------------------------------------------------------

    _run(argv, failed) {
        let process;
        try {
            process = Gio.Subprocess.new(argv, Gio.SubprocessFlags.STDOUT_SILENCE | Gio.SubprocessFlags.STDERR_SILENCE);
        } catch (e) {
            console.warn(`Amethystora UI: ${argv[0]} did not start: ${e.message}`);
            failed?.();
            return;
        }
        process.wait_check_async(this._cancellable, (p, result) => {
            let good = false;
            try {
                good = p.wait_check_finish(result);
            } catch (e) {
                // The screen was locked or the key turned off meanwhile: nobody is left to tell
                if (e.matches?.(Gio.IOErrorEnum, Gio.IOErrorEnum.CANCELLED))
                    return;
            }
            if (!good)
                failed?.();
            if (this._cancellable)
                this._askSoon();
        });
    }

    // Every job of this account the pill follows: nobody else's is ever touched, and CUPS refuses them anyway
    cancel() {
        this._cancelling = true;
        for (const id of JOBS.keys()) {
            this._run([CANCEL, String(id)], () => {
                this._cancelling = false;
                this._notice(_('The print job could not be cancelled'), 'printer-error-symbolic', LAST_WORD_S);
            });
        }
    }

    // Starting a stopped printer takes an administrator's account: anyone else is told, and offered Settings' Printers,
    // which asks for a password itself. Letting a held or stopped job go takes no one's: if that fails, it says so.
    retry() {
        const argv = this._retry();
        if (!argv)
            return;
        FOLLOW.changedAt = now();
        if (argv[0].endsWith('cupsenable')) {
            this._run(argv, () => this._notice(_('Only an administrator can start it'),
                'printer-error-symbolic', LAST_WORD_S, '', () => Util.spawn(['gnome-control-center', 'printers'])));
        } else {
            this._run(argv, () => this._notice(_('The print job could not be started again'),
                'printer-error-symbolic', LAST_WORD_S));
        }
    }

    // --- For the suites and the harness -----------------------------------------------------------------------------

    state() {
        return {
            shown: this.shown,
            id: this.id,
            text: this.shown ? this.text : '',
            trouble: trouble(this._view),
            jobs: JOBS.size,
            plate: this.plate,
            retry: this._retry(),
            timer: !!this._next,
        };
    }

    // As if CUPS had said this of job 1: an answer as ipptool -c prints it for one job (or its view), or null for the job
    // being over
    forTest(answer, last = null) {
        const view = typeof answer === 'string' ? parseState(answer) : answer;
        this._stopTimers();
        if (answer === null) {
            JOBS.clear();
            LAST.clear();
            this._apply([], last ? [last] : []);
            return this.state();
        }
        JOBS.set(1, 'Stand-in');
        this._stuck = false;
        this._show();
        this._apply([{id: 1, printer: 'Stand-in', view}], []);
        this._stopTimers();
        return this.state();
    }

    // As if the job had stood still for this long (seconds)
    standingForTest(seconds) {
        this._stopTimers();
        const seen = this._first ? [{id: this._first.id, printer: this._first.printer, view: this._first.view}] : [];
        FOLLOW.signature = JSON.stringify(seen);
        FOLLOW.changedAt = now() - seconds * 1000;
        this._apply(seen, []);
        const state = this.state();
        this._stopTimers();
        return state;
    }
}
