// "Getting set": for an account's first four weeks, the calendar menu lists the few steps that protect the machine and
// make it the person's own: a look, automatic updates, a backup, the disk asking for its PIN, a paired phone. Each line
// is ticked by what the machine already says (a file a step leaves, systemd, Backups' status, the security report,
// the phone link's list of phones), never by clicking it; a click opens the place that does it. Hide is for ever (it
// turns the key off), and once every line is done the list is gone.
//
// Nothing runs while the menu is closed. When it opens, and only while the key is on, the account is young and a line
// is left, the lines are read: two small files and one question to systemd, and two of the image's helpers run as
// the account (Backups' status and the security report), each at most once every ten minutes and never again once
// its line is done or left out. A helper asked goes on after the menu closes (its answer is kept for the next opening)
// for a minute at most, under timeout(1), which stops it and every program it started; one that did not answer in
// time is asked again at the next opening. The security line only says what the report already says and opens
// Security: it switches nothing, and is left out when the report cannot tell.

import Cairo from 'cairo';
import Clutter from 'gi://Clutter';
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import St from 'gi://St';

import {gettext as _, ngettext} from 'resource:///org/gnome/shell/extensions/extension.js';
import * as Util from 'resource:///org/gnome/shell/misc/util.js';

Gio._promisify(Gio.Subprocess.prototype, 'communicate_utf8_async');

// An account is new for this long after its home folder was made (s)
const WEEKS = 28 * 86400;
// A helper is asked again at most this often (ms)
const AGAIN = 10 * 60 * 1000;
// A helper that has not answered in this long is stopped, with everything it started (timeout(1) signals its group)
const TIMEOUT = ['/usr/bin/timeout', '--kill-after=5', '60'];
// timeout(1)'s exit status when it had to stop the helper
const TIMED_OUT = [124, 137];
const RESTORE = '/usr/libexec/amethystora-restore';
const REPORT = '/usr/libexec/amethystora-security-status';

// What each line knows, kept for the session (the lock turns the module off and on): id -> {state, at}, state one of
// 'done', 'todo', 'out' (left out); a line not in it yet is not shown
const known = new Map();

const decoder = new TextDecoder();

function readText(path) {
    try {
        const [ok, bytes] = GLib.file_get_contents(path);
        return ok ? decoder.decode(bytes) : null;
    } catch {
        return null;
    }
}

const configDir = () => GLib.get_user_config_dir();
const stateDir = () => GLib.get_user_state_dir();

// The lines, in the board's order: id, the line, why, where it opens and what opens it
function lines() {
    return [
        {id: 'look', name: _('Pick a look'), why: _('Theme, style and dock'), where: _('Control'),
            argv: ['/usr/bin/amethystora-control', 'welcome']},
        {id: 'updates', name: _('Automatic updates'), why: _('On as the image ships'), where: _('Updates'),
            argv: ['/usr/bin/amethystora-update']},
        {id: 'backup', name: _('Back up your home folder'), why: _('A disk or a server, daily'), where: _('Backups'),
            argv: ['/usr/bin/amethystora-backups']},
        {id: 'disk', name: _('A PIN to unlock the disk'), why: _('Stops a stolen laptop opening'), where: _('Security'),
            argv: ['/usr/bin/amethystora-security']},
        {id: 'phone', name: _('Pair your phone'), why: _('Files, texts, a ring when it is lost'), where: _('Phone'),
            argv: ['/usr/bin/amethystora-phone']},
    ];
}

// Whether the account's home folder was made in the last four weeks; a folder whose birth is not kept counts as old
export function young(home = GLib.get_home_dir()) {
    try {
        const info = Gio.File.new_for_path(home).query_info('time::created', Gio.FileQueryInfoFlags.NONE, null);
        if (!info.has_attribute('time::created'))
            return false;
        const born = info.get_attribute_uint64('time::created');
        return born > 0 && Date.now() / 1000 - born < WEEKS;
    } catch {
        return false;
    }
}

// The look: Pick a look was shown, or the theme or the style is the person's own
function lookDone() {
    if (GLib.file_test(GLib.build_filenamev([stateDir(), 'amethystora', 'first-look.json']), GLib.FileTest.EXISTS))
        return true;
    if (GLib.file_test(GLib.build_filenamev([configDir(), 'amethystora', 'style']), GLib.FileTest.EXISTS))
        return true;
    const theme = readText(GLib.build_filenamev([configDir(), 'amethystora', 'current', 'theme.name']))?.trim();
    return !!theme && theme !== 'amethystora';
}

// The phone link's list of phones it is paired with (the program keeps it; the shell only counts it)
function phoneDone() {
    try {
        const file = JSON.parse(readText(GLib.build_filenamev([configDir(), 'amethystora', 'connect', 'devices.json'])));
        const devices = file?.devices;
        return Array.isArray(devices) ? devices.length > 0 : Object.keys(devices ?? {}).length > 0;
    } catch {
        return false;
    }
}

// What the security report says of the disk: the TPM with a PIN, or the passphrase asked at every start, is done; a
// TPM without a PIN is to do; anything the report cannot tell (no encryption, not an administrator) is left out
export function diskState(report) {
    const checks = Array.isArray(report?.checks) ? report.checks : [];
    const pin = checks.find(c => c?.id === 'disk-unlock-pin');
    if (pin)
        return pin.state === 'ok' || pin.accepted ? 'done' : 'todo';
    const unlock = checks.find(c => c?.id === 'disk-unlock');
    return unlock?.state === 'off' ? 'done' : 'out';
}

// What Backups says: set up, and a backup has succeeded
export function backupState(status) {
    return status?.backup?.set_up === true && Number(status.backup.last_success) > 0 ? 'done' : 'todo';
}

export class GettingSet {
    constructor(dateMenu, settings) {
        this._dateMenu = dateMenu;
        this._settings = settings;
    }

    enable() {
        // A helper running, with the timer that stops it after a minute
        this._running = new Map();
        this._settings.connectObject('changed::getting-set', () => this._opened(this._dateMenu.menu.isOpen), this);
        this._dateMenu.menu.connectObject('open-state-changed', (menu, open) => this._opened(open), this);
        if (this._dateMenu.menu.isOpen)
            this._opened(true);
    }

    disable() {
        this._settings.disconnectObject(this);
        this._dateMenu.menu.disconnectObject(this);
        this._stop();
        this._card?.destroy();
        this._card = null;
        this._progress = null;
        this._rows = null;
    }

    get wanted() {
        return this._settings.get_boolean('getting-set') && (this._forceYoung ?? young());
    }

    _opened(open) {
        if (!open || !this.wanted) {
            // A helper already asked finishes (its answer is kept for the next opening), within its minute
            this._cancellable?.cancel();
            this._cancellable = null;
            if (this._card)
                this._card.visible = false;
            // Hidden for good: nothing of it is kept
            if (!this._settings.get_boolean('getting-set')) {
                this._stop();
                this._card?.destroy();
                this._card = null;
                this._progress = null;
                this._rows = null;
            }
            return;
        }
        this._build();
        this._read();
        this._show();
    }

    // Helpers still running are stopped: the module stops, or Getting set was hidden
    _stop() {
        this._cancellable?.cancel();
        this._cancellable = null;
        // timeout(1) passes the signal on to the helper's whole group
        for (const process of this._running?.keys() ?? [])
            process.send_signal(15);
        this._running = new Map();
    }

    // --- The card -------------------------------------------------------------------------------------------------

    _build() {
        if (this._card)
            return;
        const displays = this._dateMenu._eventsItem?.get_parent();
        if (!displays)
            throw new Error('the calendar menu has no events section');
        const card = new St.BoxLayout({
            style_class: 'amethystora-today-getting',
            orientation: Clutter.Orientation.VERTICAL,
            x_expand: true,
            visible: false,
        });
        const head = new St.BoxLayout({style_class: 'amethystora-today-getting-head'});
        this._ring = new St.DrawingArea({style_class: 'amethystora-today-getting-ring', y_align: Clutter.ActorAlign.CENTER});
        this._ring.connect('repaint', area => this._paintRing(area));
        head.add_child(this._ring);
        const words = new St.BoxLayout({orientation: Clutter.Orientation.VERTICAL, x_expand: true,
            y_align: Clutter.ActorAlign.CENTER});
        words.add_child(new St.Label({style_class: 'amethystora-today-getting-title', text: _('Getting set')}));
        this._progress = new St.Label({style_class: 'amethystora-today-getting-progress'});
        words.add_child(this._progress);
        head.add_child(words);
        const hide = new St.Button({
            style_class: 'amethystora-today-getting-hide',
            label: _('Hide'),
            can_focus: true,
            y_align: Clutter.ActorAlign.CENTER,
            accessible_name: _('Hide Getting set for good'),
        });
        hide.connect('clicked', () => this._settings.set_boolean('getting-set', false));
        head.add_child(hide);
        card.add_child(head);

        this._rows = new Map();
        for (const line of lines()) {
            const row = new St.Button({style_class: 'amethystora-today-getting-row', can_focus: true, x_expand: true});
            const box = new St.BoxLayout({style_class: 'amethystora-today-getting-row-box', x_expand: true});
            const mark = new St.Bin({style_class: 'amethystora-today-getting-box', y_align: Clutter.ActorAlign.CENTER});
            const tick = new St.Icon({icon_name: 'object-select-symbolic', icon_size: 14, visible: false});
            mark.set_child(tick);
            box.add_child(mark);
            const text = new St.BoxLayout({orientation: Clutter.Orientation.VERTICAL, x_expand: true,
                y_align: Clutter.ActorAlign.CENTER});
            text.add_child(new St.Label({style_class: 'amethystora-today-getting-name', text: line.name}));
            text.add_child(new St.Label({style_class: 'amethystora-today-getting-why', text: line.why}));
            box.add_child(text);
            box.add_child(new St.Label({style_class: 'amethystora-today-getting-where', text: line.where,
                y_align: Clutter.ActorAlign.CENTER}));
            row.set_child(box);
            row.connect('clicked', () => {
                this._dateMenu.menu.close();
                try {
                    Util.spawn(line.argv);
                } catch (e) {
                    console.warn(`Amethystora UI: ${line.argv[0]} did not start: ${e.message}`);
                }
            });
            card.add_child(row);
            this._rows.set(line.id, {row, mark, tick, line});
        }
        displays.insert_child_at_index(card, 0);
        this._card = card;
    }

    _paintRing(area) {
        const cr = area.get_context();
        const [w, h] = area.get_surface_size();
        const node = area.get_theme_node();
        const color = node.get_foreground_color();
        const [found, track] = node.lookup_color('-amethystora-today-track', false);
        const scale = w / 44;
        const r = 18 * scale;
        cr.setLineWidth(5 * scale);
        cr.setLineCap(Cairo.LineCap.ROUND);
        const set = c => cr.setSourceRGBA(c.red / 255, c.green / 255, c.blue / 255, c.alpha / 255);
        if (found)
            set(track);
        else
            cr.setSourceRGBA(color.red / 255, color.green / 255, color.blue / 255, 0.14);
        cr.arc(w / 2, h / 2, r, 0, 2 * Math.PI);
        cr.stroke();
        const {done, total} = this._count();
        if (total > 0 && done > 0) {
            set(color);
            cr.arc(w / 2, h / 2, r, -Math.PI / 2, -Math.PI / 2 + 2 * Math.PI * done / total);
            cr.stroke();
        }
        cr.$dispose();
    }

    _count() {
        let done = 0, total = 0;
        for (const id of this._rows?.keys() ?? []) {
            const state = known.get(id)?.state ?? 'wait';
            if (state === 'out' || state === 'wait')
                continue;
            total++;
            if (state === 'done')
                done++;
        }
        return {done, total};
    }

    _show() {
        if (!this._card)
            return;
        for (const [id, {row, mark, tick, line}] of this._rows) {
            // A line not known yet is not shown: it may be one the machine has nothing to do about
            const state = known.get(id)?.state ?? 'wait';
            row.visible = state === 'done' || state === 'todo';
            tick.visible = state === 'done';
            if (state === 'done')
                mark.add_style_pseudo_class('checked');
            else
                mark.remove_style_pseudo_class('checked');
            row.accessible_name = state === 'done'
                ? _('%s, done. Opens %s').format(line.name, line.where)
                : _('%s, to do. Opens %s').format(line.name, line.where);
        }
        const {done, total} = this._count();
        this._progress.text = ngettext('%d of %d done', '%d of %d done', total).format(done, total);
        this._ring.queue_repaint();
        // Everything done: nothing to show
        this._card.visible = this._dateMenu.menu.isOpen && this.wanted && total > done;
    }

    // --- Reading the lines ----------------------------------------------------------------------------------------

    _set(id, state) {
        known.set(id, {state, at: Date.now()});
        this._show();
    }

    // A helper is asked when its line is not known, or still to do and its answer is ten minutes old; never while
    // it runs, never again once done or left out
    _due(id) {
        if ([...this._running.values()].includes(id))
            return false;
        const last = known.get(id);
        return !last || (last.state === 'todo' && Date.now() - last.at > AGAIN);
    }

    _read() {
        this._cancellable?.cancel();
        const cancellable = new Gio.Cancellable();
        this._cancellable = cancellable;
        this._set('look', lookDone() ? 'done' : 'todo');
        this._set('phone', phoneDone() ? 'done' : 'todo');
        Gio.DBus.system.call('org.freedesktop.systemd1', '/org/freedesktop/systemd1',
            'org.freedesktop.systemd1.Manager', 'GetUnitFileState', new GLib.Variant('(s)', ['uupd.timer']),
            new GLib.VariantType('(s)'), Gio.DBusCallFlags.NO_AUTO_START, 2000, cancellable, (connection, result) => {
                let state;
                try {
                    const [value] = connection.call_finish(result).deepUnpack();
                    state = value === 'enabled' ? 'done' : 'todo';
                } catch (e) {
                    if (e.matches?.(Gio.IOErrorEnum, Gio.IOErrorEnum.CANCELLED))
                        return;
                    // No such timer here: nothing to do about it
                    state = 'out';
                }
                this._set('updates', state);
            });
        if (this._due('backup'))
            this._ask('backup', [RESTORE, 'status', '--json'], backupState);
        if (this._due('disk'))
            this._ask('disk', [REPORT, '--json'], diskState);
    }

    // One of the image's helpers, as the account, for a minute at most; its answer is kept for the session
    // One of the image's helpers, as the account, under timeout(1); its answer is kept for the session
    async _ask(id, argv, decide) {
        if (!GLib.file_test(argv[0], GLib.FileTest.IS_EXECUTABLE)) {
            this._set(id, 'out');
            return;
        }
        let process = null;
        try {
            process = Gio.Subprocess.new([...TIMEOUT, ...argv],
                Gio.SubprocessFlags.STDOUT_PIPE | Gio.SubprocessFlags.STDERR_SILENCE);
            this._running.set(process, id);
            const [out] = await process.communicate_utf8_async(null, null);
            this._running?.delete(process);
            // Stopped for taking too long, or with the module: asked again at the next opening
            if (TIMED_OUT.includes(process.get_exit_status()) || process.get_if_signaled())
                return;
            if (this._rows)
                this._set(id, decide(JSON.parse(out)));
        } catch (e) {
            this._running?.delete(process);
            if (this._rows) {
                console.debug(`Amethystora UI: ${argv[0]} did not answer: ${e.message}`);
                this._set(id, 'out');
            }
        }
    }

    // --- For the suites -------------------------------------------------------------------------------------------

    // The lines as shown: [{id, state}], and whether the card shows
    state() {
        return {
            shown: !!this._card?.visible,
            progress: this._progress?.text ?? '',
            lines: [...this._rows?.keys() ?? []].map(id => ({id, state: known.get(id)?.state ?? 'wait'})),
        };
    }

    // An account of any age (the lab's tester may be older than four weeks)
    forceYoungForTest(on) {
        this._forceYoung = on ? true : null;
        this._opened(this._dateMenu.menu.isOpen);
    }
}
