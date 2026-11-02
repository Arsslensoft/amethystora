// Never out of room (R7-58): when GNOME's own low-space notice fires, it is replaced by one that says how much can be cleared
// and that none of it is the person's work, with one button. There is no timer here: GNOME's housekeeping decides when space is
// low, and a notice this did not recognize is left alone. /usr/libexec/amethystora-free-space works out the totals and clears;
// nothing is written, and the words hold totals and no name. A lock disables this module and GNOME says it again only once the
// disk has lost another percent, so the offer, its actions and what says when it may be asked again are kept at module scope.

import Clutter from 'gi://Clutter';
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import GObject from 'gi://GObject';
import Pango from 'gi://Pango';
import St from 'gi://St';

import {gettext as _, ngettext} from 'resource:///org/gnome/shell/extensions/extension.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import * as MessageTray from 'resource:///org/gnome/shell/ui/messageTray.js';
import * as ModalDialog from 'resource:///org/gnome/shell/ui/modalDialog.js';

import {run} from './run.js';

const HELPER = '/usr/libexec/amethystora-free-space';
const ICON = 'drive-harddisk-symbolic';
// What is not worth a notice of its own: GNOME's stays
const SMALLEST = 200 * 1000 * 1000;
// GNOME says it again at most every ten minutes by default; a program that sends a look-alike gets no more than that
const AT_MOST_EVERY = 10 * 60 * 1000 * 1000;
const CHECK = 3 * 60 * 1000;
const CLEAR = 30 * 60 * 1000;

let source = null;
let offer = null;
let dialog = null;
let clearing = false;
let lastCheck = 0;

function size(bytes) {
    return GLib.format_size(Math.max(0, bytes));
}

// GNOME's notice, as gnome-settings-daemon's housekeeping sends it: critical, passing, with its drive picture
function isLowSpaceNotice(notification) {
    return notification.urgency === MessageTray.Urgency.CRITICAL && notification.isTransient &&
        notification.gicon?.to_string?.() === ICON;
}

// GNOME's words are translated, and never parsed, but a notice about a stick or a network folder names it: that one is GNOME's to
// say, whatever the home's disk has. A sentence that only happens to hold such a name stays GNOME's too.
function namesAnotherDisk(notification) {
    const words = `${notification.title} ${notification.body}`;
    return Gio.VolumeMonitor.get().get_mounts().some(mount => words.includes(mount.get_name()));
}

function ensureSource() {
    if (!source) {
        source = new MessageTray.Source({title: _('Disk Space'), iconName: ICON});
        source.connect('destroy', () => (source = null));
        Main.messageTray.add(source);
    }
    return source;
}

function say(title, body) {
    const from = ensureSource();
    const notification = new MessageTray.Notification({source: from, title, body, isTransient: true});
    from.addNotification(notification);
    return notification;
}

// One offer at a time: the last one's total is out of date
function makeOffer(found) {
    offer?.destroy();
    const from = ensureSource();
    const notification = new MessageTray.Notification({
        source: from,
        title: _('Nearly out of room'),
        body: _('%s left. %s can be cleared, and none of it is your work.').format(size(found.free), size(found.total)),
        urgency: MessageTray.Urgency.CRITICAL,
    });
    notification.addAction(_('Show me first'), () => showFirst(found));
    notification.addAction(_('Clear %s').format(size(found.total)), () => clearRoom(found));
    notification.connect('destroy', () => {
        if (offer === notification)
            offer = null;
    });
    offer = notification;
    from.addNotification(notification);
}

function showFirst(found) {
    dialog?.close();
    const made = new RoomDialog(found, () => clearRoom(found));
    made.connect('destroy', () => {
        if (dialog === made)
            dialog = null;
    });
    dialog = made;
    made.open();
}

async function clearRoom(found) {
    if (clearing)
        return;
    clearing = true;
    offer?.destroy();
    let working = say(_('Clearing space…'), '');
    working.connect('destroy', () => (working = null));
    let done = null;
    try {
        const {ok, stdout} = await run([HELPER, 'clear', '--json'], {timeout: CLEAR});
        done = ok ? JSON.parse(stdout) : null;
    } catch (e) {
        console.warn(`Amethystora UI: the space was not cleared: ${e.message}`);
    }
    clearing = false;
    working?.destroy();
    if (!done) {
        say(_('The space could not be cleared'), _('Free disk space in Control asks about each thing.'));
        return;
    }
    const later = found.snapshots && found.home_part > 0
        ? _('What was in your home folder comes free as the hourly snapshots expire, within 14 days.') : '';
    say(_('Cleared %s').format(size(done.cleared)), [_('%s free now.').format(size(done.free)), later].filter(part => part).join(' '));
}

const RoomDialog = GObject.registerClass(
class RoomDialog extends ModalDialog.ModalDialog {
    _init(found, clear) {
        super._init({styleClass: 'amethystora-tools-users-dialog', destroyOnClose: true});
        const line = (text, styleClass) => {
            const made = new St.Label({text, style_class: styleClass});
            made.clutter_text.line_wrap = true;
            made.clutter_text.line_wrap_mode = Pango.WrapMode.WORD_CHAR;
            return made;
        };
        this.contentLayout.add_child(line(_('Nearly out of room'), 'amethystora-tools-users-title'));
        this.contentLayout.add_child(line(_('%s left. This is safe to clear, and none of it is yours:').format(size(found.free)), 'amethystora-tools-users-detail'));
        const rows = new St.BoxLayout({style_class: 'amethystora-tools-users', orientation: Clutter.Orientation.VERTICAL, x_expand: true});
        const row = (name, detail) => {
            const box = new St.BoxLayout({style_class: 'amethystora-tools-users-row'});
            const words = new St.BoxLayout({orientation: Clutter.Orientation.VERTICAL, x_expand: true, y_align: Clutter.ActorAlign.CENTER});
            words.add_child(new St.Label({text: name, style_class: 'amethystora-tools-users-name'}));
            words.add_child(new St.Label({text: detail, style_class: 'amethystora-tools-users-detail'}));
            box.add_child(words);
            rows.add_child(box);
        };
        if (found.apps > 0)
            row(_('Old app versions and caches'), size(found.apps));
        // The days are the account's own housekeeping's, and with that off the trash is kept until it is emptied
        const days = found.trash_days;
        if (found.trash > 0)
            row(_('The trash'), ngettext('%s, older than %d day', '%s, older than %d days', days).format(size(found.trash), days));
        if (found.trash_recent > 0) {
            const text = Number.isInteger(days)
                ? ngettext('Another %s has been in the trash for less than %d day. That is yours to empty.',
                    'Another %s has been in the trash for less than %d days. That is yours to empty.', days).format(size(found.trash_recent), days)
                : _('Your trash holds %s, which stays until you empty it.').format(size(found.trash_recent));
            rows.add_child(line(text, 'amethystora-tools-users-footer'));
        }
        if (found.snapshots && found.home_part > 0)
            rows.add_child(line(_('The hourly snapshots still hold what was in your home folder, so that part comes free as they expire, within 14 days.'), 'amethystora-tools-users-footer'));
        this.contentLayout.add_child(rows);
        this.setButtons([
            {label: _('Close'), action: () => this.close(), key: Clutter.KEY_Escape},
            {
                label: _('Choose each myself'),
                action: () => {
                    this.close();
                    try {
                        Gio.Subprocess.new(['/usr/bin/amethystora-control', 'system', 'clean'], Gio.SubprocessFlags.NONE);
                    } catch (e) {
                        console.warn(`Amethystora UI: Control did not start: ${e.message}`);
                    }
                },
            },
            {
                label: _('Clear %s').format(size(found.total)),
                default: true,
                action: () => {
                    this.close();
                    clear();
                },
            },
        ]);
    }
});

export class Room {
    constructor(settings) {
        this._settings = settings;
    }

    enable() {
        this._settings.connectObject('changed::offer-room', () => this._apply(), this);
        this._apply();
    }

    disable() {
        this._settings?.disconnectObject(this);
        this._unwatch();
        this._cancellable?.cancel();
        this._cancellable = null;
        dialog?.close();
        this._settings = null;
    }

    // Off, nothing here is connected and GNOME's notice is as it was
    _apply() {
        if (this._settings.get_boolean('offer-room'))
            this._watch();
        else
            this._unwatch();
    }

    _watch() {
        if (this._watching)
            return;
        this._watching = true;
        Main.messageTray.connectObject('source-added', (tray, source) => this._watchSource(source), this);
        for (const source of Main.messageTray.getSources())
            this._watchSource(source);
    }

    _unwatch() {
        if (!this._watching)
            return;
        this._watching = false;
        Main.messageTray.disconnectObject(this);
        for (const source of Main.messageTray.getSources())
            source.disconnectObject(this);
    }

    _watchSource(source) {
        source.connectObject('notification-added', (s, notification) => this._added(notification), this);
    }

    _added(notification) {
        if (!isLowSpaceNotice(notification) || namesAnotherDisk(notification))
            return;
        const now = GLib.get_monotonic_time();
        if (this._checking || (lastCheck && now - lastCheck < AT_MOST_EVERY))
            return;
        lastCheck = now;
        this._check(notification).catch(e => {
            if (!e.matches?.(Gio.IOErrorEnum, Gio.IOErrorEnum.CANCELLED))
                console.warn(`Amethystora UI: what can be cleared could not be read: ${e.message}`);
        });
    }

    async _check(gnomeNotice) {
        this._checking = true;
        let gone = false;
        gnomeNotice.connectObject('destroy', () => (gone = true), this);
        try {
            this._cancellable ??= new Gio.Cancellable();
            const {ok, stdout} = await run([HELPER, 'check', '--json'], {timeout: CHECK, cancellable: this._cancellable});
            const found = ok ? JSON.parse(stdout) : null;
            // Turned off meanwhile, nothing worth offering, or another disk than the one the home and the apps are on: GNOME's
            // notice says it
            if (!this._settings?.get_boolean('offer-room') || !found?.low || !(found.total >= SMALLEST))
                return;
            if (!gone)
                gnomeNotice.destroy();
            makeOffer(found);
        } finally {
            this._checking = false;
            if (!gone)
                gnomeNotice.disconnectObject(this);
        }
    }

    // For the checklist
    get state() {
        return {watching: !!this._watching, checking: !!this._checking, clearing, offer: !!offer};
    }
}
