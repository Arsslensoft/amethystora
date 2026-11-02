// Your year, on this machine (R7-30; offered): around the turn of the year, a card in Today offers one page about the
// year on this computer. It is worked out by /usr/libexec/amethystora-your-year when it is opened, shown, and gone when it
// closes: nothing is collected for it, and nothing but the year it was last offered for is kept (a key). The helper runs
// only after the click; the page shows at once and fills in. A line it cannot read is left out.

import Clutter from 'gi://Clutter';
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import Shell from 'gi://Shell';
import St from 'gi://St';

import {gettext as _, ngettext} from 'resource:///org/gnome/shell/extensions/extension.js';
import * as ModalDialog from 'resource:///org/gnome/shell/ui/modalDialog.js';

import {offeredYear} from './yearwindow.js';

Gio._promisify(Gio.Subprocess.prototype, 'communicate_utf8_async');
Gio._promisify(Gio.File.prototype, 'query_info_async');

const HELPER = '/usr/libexec/amethystora-your-year';
const ASK_MS = 30000;
const MOST_APPS = 3;

function row(parent, title) {
    const box = new St.BoxLayout({orientation: Clutter.Orientation.VERTICAL, style_class: 'amethystora-today-year-row', visible: false});
    box.add_child(new St.Label({text: title, style_class: 'amethystora-today-year-name'}));
    const value = new St.Label({style_class: 'amethystora-today-year-value'});
    value.clutter_text.use_markup = false;
    value.clutter_text.line_wrap = true;
    box.add_child(value);
    parent.add_child(box);
    return {
        set(text) {
            value.text = text;
            box.visible = Boolean(text);
        },
    };
}

class YearPage {
    constructor(year) {
        this._cancellable = new Gio.Cancellable();
        this.dialog = new ModalDialog.ModalDialog({styleClass: 'amethystora-today-year-dialog'});
        this.dialog.connect('destroy', () => this._cancellable.cancel());
        const box = new St.BoxLayout({orientation: Clutter.Orientation.VERTICAL, style_class: 'amethystora-today-year'});
        box.add_child(new St.Label({text: _('Your year on Amethystora, %d').format(year), style_class: 'amethystora-today-year-title'}));
        this._rows = {
            days: row(box, _('Together for')),
            apps: row(box, _('Used most')),
            snapshots: row(box, _('Snapshots')),
            backup: row(box, _('Last backed up')),
            findings: row(box, _('The security watch')),
        };
        const foot = new St.Label({text: _('Worked out here. Nothing was sent.'), style_class: 'amethystora-today-year-foot'});
        box.add_child(foot);
        this.dialog.contentLayout.add_child(box);
        this.dialog.setButtons([{label: _('Close'), action: () => this.dialog.close(), default: true, key: Clutter.KEY_Escape}]);
        this.dialog.open();
        // GNOME's own record of use, which it holds in memory
        const apps = Shell.AppUsage.get_default().get_most_used().filter(app => app.get_app_info()?.should_show())
            .slice(0, MOST_APPS).map(app => app.get_name());
        this._rows.apps.set(apps.join(', '));
        this._ask().catch(e => console.debug(`Amethystora UI: your year could not be read: ${e.message}`));
    }

    async _ask() {
        const process = Gio.Subprocess.new([HELPER, '--json'], Gio.SubprocessFlags.STDOUT_PIPE | Gio.SubprocessFlags.STDERR_SILENCE);
        const stop = GLib.timeout_add(GLib.PRIORITY_DEFAULT, ASK_MS, () => {
            process.force_exit();
            return GLib.SOURCE_REMOVE;
        });
        let out;
        try {
            [out] = await process.communicate_utf8_async(null, this._cancellable);
        } finally {
            GLib.source_remove(stop);
        }
        if (this._cancellable.is_cancelled())
            return;
        const answer = JSON.parse(out);
        const whole = value => Number.isInteger(value) && value >= 0;
        if (whole(answer.days))
            this._rows.days.set(ngettext('%d day', '%d days', answer.days).format(answer.days));
        // A row of its own for each, every one a whole sentence or date: none is built from pieces
        if (whole(answer.snapshots) && answer.snapshots > 0) {
            this._rows.snapshots.set(ngettext('%d snapshot of your home folder', '%d snapshots of your home folder',
                answer.snapshots).format(answer.snapshots));
        }
        if (whole(answer.last_backup) && answer.last_backup > 0)
            this._rows.backup.set(GLib.DateTime.new_from_unix_local(answer.last_backup).format('%x'));
        // The watch keeps only its most recent lines, so this is of lately and not of the year
        if (whole(answer.findings)) {
            this._rows.findings.set(ngettext('%d thing it told you about lately', '%d things it told you about lately',
                answer.findings).format(answer.findings));
        }
    }
}

export class YourYear {
    constructor(dateMenu, settings) {
        this._dateMenu = dateMenu;
        this._settings = settings;
    }

    enable() {
        this._settings.connectObject('changed::your-year', () => this._sync(), this);
        this._sync();
    }

    disable() {
        this._settings.disconnectObject(this);
        this._settings = null;
        this._dateMenu.menu.disconnectObject(this);
        this._page?.dialog.close();
        this._page = null;
        this._card?.destroy();
        this._card = null;
    }

    // Listens to the menu only while the key is on; nothing else runs
    _sync() {
        this._dateMenu.menu.disconnectObject(this);
        if (this._settings.get_boolean('your-year')) {
            this._dateMenu.menu.connectObject('open-state-changed', (menu, open) => this._opened(open), this);
        } else {
            this._card?.destroy();
            this._card = null;
        }
    }

    async _born() {
        try {
            const info = await Gio.File.new_for_path(GLib.get_home_dir()).query_info_async('time::created',
                Gio.FileQueryInfoFlags.NONE, GLib.PRIORITY_DEFAULT, null);
            return info.has_attribute('time::created') ? info.get_attribute_uint64('time::created') : 0;
        } catch {
            return 0;
        }
    }

    // Outside the weeks around the turn of the year this is two comparisons; only inside them does it look at the home
    async _opened(open) {
        if (!open)
            return;
        const seen = this._settings.get_int('your-year-seen');
        let year = this._forceYear ?? offeredYear(new Date(), 0, seen);
        if (year && !this._forceYear) {
            const born = await this._born();
            if (!this._settings)
                return;
            year = offeredYear(new Date(), born, seen);
        }
        this._year = year;
        if (!this._year) {
            this._card?.destroy();
            this._card = null;
            return;
        }
        if (this._dateMenu.menu.isOpen)
            this._build();
    }

    _seen() {
        this._settings.set_int('your-year-seen', this._year);
        this._card?.destroy();
        this._card = null;
    }

    _build() {
        if (this._card)
            return;
        const displays = this._dateMenu._eventsItem?.get_parent();
        if (!displays)
            return;
        const card = new St.BoxLayout({style_class: 'amethystora-today-getting', orientation: Clutter.Orientation.VERTICAL, x_expand: true});
        card.add_child(new St.Label({style_class: 'amethystora-today-getting-title', text: _('Your year, on this machine')}));
        const text = new St.Label({style_class: 'amethystora-today-getting-progress', text: _('One page, worked out here when you open it, and never kept.')});
        text.clutter_text.line_wrap = true;
        card.add_child(text);
        const buttons = new St.BoxLayout({style_class: 'amethystora-today-year-buttons', x_align: Clutter.ActorAlign.END});
        const skip = new St.Button({style_class: 'amethystora-today-getting-hide', label: _('Not this year'), can_focus: true});
        skip.connect('clicked', () => this._seen());
        buttons.add_child(skip);
        const look = new St.Button({style_class: 'amethystora-today-getting-hide', label: _('Open'), can_focus: true});
        look.connect('clicked', () => {
            const year = this._year;
            this._dateMenu.menu.close();
            this._seen();
            this._page = new YearPage(year);
            this._page.dialog.connect('destroy', () => {
                this._page = null;
            });
        });
        buttons.add_child(look);
        card.add_child(buttons);
        displays.insert_child_at_index(card, 0);
        this._card = card;
    }

    // For the tests
    offerForTest(year) {
        this._forceYear = year;
    }
}
