// Update ready: a new version of the system is staged and waits for a restart. ostree says so with a file in
// /run/ostree, which is watched (never polled); whether the restart can be a quick one is asked of the image's own
// helper, once for each staged version. Restart now opens System Updates, which asks before it restarts anything;
// Tonight puts it out of sight until eight in the evening.

import Clutter from 'gi://Clutter';
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import Shell from 'gi://Shell';
import St from 'gi://St';

import {gettext as _} from 'resource:///org/gnome/shell/extensions/extension.js';

import {buttonRow, cardButton, retire} from './clock.js';

const RUN = '/run';
const OSTREE = '/run/ostree';
const STAGED = 'staged-deployment';
// bootc's download-only staging, which is not ready to restart into
const LOCKED = 'staged-deployment-locked';
const SEALED = '/usr/share/amethystora/sealed';
const SOFT_REBOOT = '/usr/libexec/amethystora-soft-reboot';
// Tonight starts at this hour
const EVENING = 20;

Gio._promisify(Gio.Subprocess.prototype, 'communicate_utf8_async');

// Tonight's end (ms since the epoch), kept outside the module so that a lock (which turns the extension off) does not
// bring the update back before eight
let hiddenUntil = 0;

export class Update {
    constructor(activities) {
        this.id = 'update';
        this.plate = '';
        this._activities = activities;
    }

    enable() {
        // A sealed machine stages its updates elsewhere: nothing is offered there yet
        if (GLib.file_test(SEALED, GLib.FileTest.EXISTS))
            return;
        this._cancellable = new Gio.Cancellable();
        this._quick = null;
        this._staged = null;
        this._armTonight();
        this._watch();
    }

    disable() {
        this._cancellable?.cancel();
        this._cancellable = null;
        this._unwatch();
        if (this._tonight)
            GLib.source_remove(this._tonight);
        this._tonight = 0;
        this._end();
    }

    // The folder ostree writes in, or, when it is not there yet, /run for it to come: a monitor on a path that does
    // not exist would make GLib look for it every few seconds
    _watch() {
        this._unwatch();
        const ostree = Gio.File.new_for_path(OSTREE);
        const watched = ostree.query_exists(null) ? ostree : Gio.File.new_for_path(RUN);
        try {
            this._monitor = watched.monitor_directory(Gio.FileMonitorFlags.NONE, this._cancellable);
        } catch (e) {
            console.warn(`Amethystora UI: the pill cannot watch ${watched.get_path()}: ${e.message}`);
            return;
        }
        this._monitor.connect('changed', (m, file) => {
            if (watched === ostree || file.get_path() === OSTREE)
                this._sync(watched !== ostree);
        });
        this._sync(false);
    }

    _unwatch() {
        this._monitor?.cancel();
        this._monitor = null;
    }

    _sync(rewatch) {
        if (rewatch && Gio.File.new_for_path(OSTREE).query_exists(null)) {
            this._watch();
            return;
        }
        const ready = GLib.file_test(`${OSTREE}/${STAGED}`, GLib.FileTest.EXISTS) &&
            !GLib.file_test(`${OSTREE}/${LOCKED}`, GLib.FileTest.EXISTS);
        if (!ready) {
            this._quick = null;
            this._staged = null;
            this._end();
            return;
        }
        // Asked again for each staged version: a second update staged over the first is another file
        const staged = this._stagedId();
        if (staged !== this._staged) {
            this._staged = staged;
            this._quick = null;
        }
        if (this._quick === null) {
            this._quick = false;
            this._ask();
        }
        this._show();
    }

    // Whether the staged version can be finished with a soft reboot, as the image's helper says
    async _ask() {
        try {
            const process = Gio.Subprocess.new([SOFT_REBOOT, 'status'],
                Gio.SubprocessFlags.STDOUT_PIPE | Gio.SubprocessFlags.STDERR_SILENCE);
            const [out] = await process.communicate_utf8_async(null, this._cancellable);
            const answer = JSON.parse(out);
            this._quick = answer?.quick === true;
        } catch (e) {
            if (!e.matches?.(Gio.IOErrorEnum, Gio.IOErrorEnum.CANCELLED))
                console.debug(`Amethystora UI: the soft reboot's helper did not answer: ${e.message}`);
            return;
        }
        if (this._activities.get('update') === this) {
            this._label.text = this.text;
            this._activities.update('update');
        }
    }

    _stagedId() {
        try {
            const info = Gio.File.new_for_path(`${OSTREE}/${STAGED}`).query_info('unix::inode,time::modified',
                Gio.FileQueryInfoFlags.NONE, null);
            return `${info.get_attribute_uint64('unix::inode')}:${info.get_attribute_uint64('time::modified')}`;
        } catch {
            return '';
        }
    }

    get _hidden() {
        return hiddenUntil > Date.now();
    }

    // The update comes back at eight, also after the module was turned off and on meanwhile
    _armTonight() {
        if (this._tonight)
            GLib.source_remove(this._tonight);
        this._tonight = 0;
        const seconds = Math.ceil((hiddenUntil - Date.now()) / 1000);
        if (seconds <= 0)
            return;
        this._tonight = GLib.timeout_add_seconds(GLib.PRIORITY_DEFAULT, seconds, () => {
            this._tonight = 0;
            hiddenUntil = 0;
            this._sync(false);
            return GLib.SOURCE_REMOVE;
        });
    }

    _show() {
        if (this._hidden)
            return;
        this.pill();
        this._label.text = this.text;
        this._activities.set('update', this);
        this._activities.update('update');
    }

    _end() {
        if (this._activities.get('update') === this)
            this._activities.set('update', null);
        retire(this._content);
        this._content = null;
    }

    get text() {
        return this._quick ? _('Ready · quick restart') : _('Ready · restart');
    }

    get accessible() {
        return _('A new version is ready');
    }

    get row() {
        return {icon: 'amethystora-update', text: this.text};
    }

    pill() {
        if (!this._content) {
            this._content = new St.BoxLayout({style_class: 'amethystora-pill-content amethystora-pill-update'});
            this._content.add_child(new St.Icon({
                icon_name: 'amethystora-update',
                fallback_icon_name: 'software-update-available-symbolic',
                icon_size: 16,
                y_align: Clutter.ActorAlign.CENTER,
            }));
            this._label = new St.Label({y_align: Clutter.ActorAlign.CENTER});
            this._content.add_child(this._label);
        }
        return this._content;
    }

    restart() {
        Shell.AppSystem.get_default().lookup_app('amethystora-update.desktop')?.activate();
    }

    // Out of sight until 20:00 today; from 20:00 on there is no such button
    tonight() {
        const now = GLib.DateTime.new_now_local();
        const evening = GLib.DateTime.new_local(now.get_year(), now.get_month(), now.get_day_of_month(), EVENING, 0, 0);
        const seconds = Math.ceil(evening.difference(now) / 1e6);
        if (seconds <= 0)
            return;
        hiddenUntil = Date.now() + seconds * 1000;
        this._armTonight();
        this._end();
    }

    card(api) {
        const body = new St.BoxLayout({style_class: 'amethystora-pill-card-body', orientation: Clutter.Orientation.VERTICAL});
        body.add_child(new St.Label({style_class: 'amethystora-pill-card-label', text: _('A new version is ready')}));
        const text = new St.Label({
            style_class: 'amethystora-pill-card-text',
            text: this._quick
                ? _('A quick restart: the system\'s programs restart, the computer does not. Save your work first.')
                : _('The computer restarts to finish it. Save your work first.'),
        });
        text.clutter_text.line_wrap = true;
        body.add_child(text);
        const buttons = [cardButton(_('Restart now'), () => {
            api.close();
            this.restart();
        }, {primary: true})];
        if (GLib.DateTime.new_now_local().get_hour() < EVENING) {
            buttons.push(cardButton(_('Tonight'), () => {
                api.close();
                this.tonight();
            }));
        }
        body.add_child(buttonRow(buttons));
        return body;
    }
}
