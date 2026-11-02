// Anything opens (R7-40): a file nothing here opens says what it is and which app on Flathub opens it, one click from installed
// and open. /usr/libexec/amethystora-open-anything, the launcher that claims those kinds of file, asks for the card over D-Bus and
// works out what it says: a caller only names a file. The card shows at once and is filled in when the helper answers, a
// few seconds on a slow disk. Nothing is kept: the file's name is in this dialog while it shows and nowhere else.

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

const HELPER = '/usr/libexec/amethystora-open-anything';
const BAZAAR = ['flatpak', 'run', 'io.github.kolunmi.Bazaar'];
const PATH = '/org/amethystora/Tools/Opener';
const INTERFACE = `
<node>
  <interface name="org.amethystora.Tools.Opener">
    <method name="Show">
      <arg type="s" direction="in" name="path"/>
    </method>
  </interface>
</node>`;
const LONGEST_PATH = 4096;
// A catalogue this size takes seconds on a slow disk, an app minutes to download
const LOOK_UP = 60000;
const INSTALL = 30 * 60 * 1000;
const REF = /^app\/[A-Za-z_][A-Za-z0-9_-]*(\.[A-Za-z_][A-Za-z0-9_-]*)+\/[A-Za-z0-9_]+\/[A-Za-z0-9_.-]+$/;

function plain(text, longest) {
    // eslint-disable-next-line no-control-regex
    return String(text ?? '').replace(/[\u0000-\u001f\u007f-\u009f‎‏‪-‮⁦-⁩]/g, '').trim().slice(0, longest);
}

function label(text, styleClass, wrap = false) {
    const made = new St.Label({text, style_class: styleClass});
    made.clutter_text.use_markup = false;
    if (wrap) {
        made.clutter_text.line_wrap = true;
        made.clutter_text.line_wrap_mode = Pango.WrapMode.WORD_CHAR;
    } else {
        made.clutter_text.ellipsize = Pango.EllipsizeMode.END;
    }
    return made;
}

const OpenerDialog = GObject.registerClass(
class OpenerDialog extends ModalDialog.ModalDialog {
    _init(path) {
        super._init({styleClass: 'amethystora-tools-users-dialog', destroyOnClose: true});
        this._path = path;
        this._cancellable = new Gio.Cancellable();
        const title = label(plain(GLib.path_get_basename(path), 80) || '?', 'amethystora-tools-users-title');
        title.clutter_text.ellipsize = Pango.EllipsizeMode.MIDDLE;
        this.contentLayout.add_child(title);
        this._kind = label('', 'amethystora-tools-users-detail', true);
        this.contentLayout.add_child(this._kind);
        this._body = new St.BoxLayout({style_class: 'amethystora-tools-users', orientation: Clutter.Orientation.VERTICAL, x_expand: true});
        this.contentLayout.add_child(this._body);
        this._status = label(_('Looking…'), 'amethystora-tools-users-status', true);
        this._body.add_child(this._status);
        this._rows = new St.BoxLayout({style_class: 'amethystora-tools-users-rows', orientation: Clutter.Orientation.VERTICAL});
        this._body.add_child(this._rows);
        this.setButtons([
            {label: _('Find an app in Bazaar'), action: () => this._openBazaar()},
            {label: _('Close'), action: () => this.close(), key: Clutter.KEY_Escape, default: true},
        ]);
        this.connect('destroy', () => this._cancellable.cancel());
        this.answer = null;
        this._read();
    }

    async _read() {
        let answer = null;
        try {
            const {ok, stdout} = await run([HELPER, 'lookup', '--json', '--', this._path], {timeout: LOOK_UP, cancellable: this._cancellable});
            answer = ok ? JSON.parse(stdout) : null;
        } catch (e) {
            if (e.matches?.(Gio.IOErrorEnum, Gio.IOErrorEnum.CANCELLED))
                return;
        }
        if (this._cancellable.is_cancelled())
            return;
        this.answer = answer;
        this._show(answer);
    }

    _show(answer) {
        if (typeof answer?.description === 'string')
            this._kind.text = plain(answer.description, 200);
        const apps = (Array.isArray(answer?.apps) ? answer.apps : []).filter(app => REF.test(app?.ref ?? ''));
        if (!answer)
            this._status.text = _('What this file is could not be read.');
        else if (answer.unknown)
            this._status.text = _('The file does not say what kind it is. Open With in Files can try any app on it.');
        else if (answer.catalogue === false)
            this._status.text = _('The list of apps is not on this computer yet. It comes with the first update online.');
        else if (apps.length === 0)
            this._status.text = _('No app on Flathub says it opens this kind of file. Open With in Files can try any app on it.');
        else if (apps.some(app => !app.installed))
            this._status.text = ngettext('Nothing here opens this yet. This app does:', 'Nothing here opens this yet. These apps do:', Math.min(apps.length, 3));
        else
            this._status.text = ngettext('This app opens it:', 'These apps open it:', Math.min(apps.length, 3));
        for (const app of apps.slice(0, 3))
            this._rows.add_child(this._row(app));
        if (apps.length > 0)
            this._body.add_child(label(_('The file stays on this computer. Only its type was looked up, in the list already here.'), 'amethystora-tools-users-footer', true));
    }

    _row(app) {
        const box = new St.BoxLayout({style_class: 'amethystora-tools-users-row'});
        const words = new St.BoxLayout({orientation: Clutter.Orientation.VERTICAL, x_expand: true, y_align: Clutter.ActorAlign.CENTER});
        const name = plain(app.name, 60);
        words.add_child(label(name, 'amethystora-tools-users-name'));
        const size = plain(app.size, 16);
        const from = app.installed ? _('Installed') : size ? _('From Flathub, %s').format(size) : _('From Flathub');
        const detail = [plain(app.summary, 120), from].filter(part => part).join(' · ');
        words.add_child(label(detail, 'amethystora-tools-users-detail'));
        // Said before the click: an app claims by itself what it opens, and installing it starts it on this file
        if (!app.installed)
            words.add_child(label(app.verified ? _('Flathub checked who made this.') : _('Flathub has not checked who made this.'), 'amethystora-tools-users-detail', true));
        box.add_child(words);
        const button = new St.Button({
            style_class: 'amethystora-tools-users-quit',
            label: app.installed ? _('Open') : _('Install and open'),
            can_focus: true,
            y_align: Clutter.ActorAlign.CENTER,
        });
        button.accessible_name = `${button.label}: ${name}`;
        button.connect('clicked', () => this._open(app, name, button));
        box.add_child(button);
        return box;
    }

    // It carries on when the card is closed: the app opens the file once it is there
    async _open(app, name, button) {
        for (const row of this._rows.get_children())
            row.get_last_child().reactive = false;
        button.label = app.installed ? _('Opening…') : _('Installing…');
        const verb = app.installed ? 'open' : 'install-open';
        // Closed while it installs, the card no longer shows that anything is going on
        const closed = app.installed ? 0 : this.connect('destroy', () => {
            const source = MessageTray.getSystemSource();
            source.addNotification(new MessageTray.Notification({
                source,
                title: _('Installing %s…').format(name),
                body: _('It opens the file when it is done.'),
                isTransient: true,
            }));
        });
        let ok = false;
        try {
            ({ok} = await run([HELPER, verb, app.ref, '--', this._path], {timeout: INSTALL}));
        } catch {}
        if (closed && !this._cancellable.is_cancelled())
            this.disconnect(closed);
        if (ok) {
            if (!this._cancellable.is_cancelled())
                this.close();
            return;
        }
        Main.notifyError(_('%s could not be installed').format(name), _('Try installing it in Bazaar.'));
        if (!this._cancellable.is_cancelled()) {
            button.label = _('Install and open');
            for (const row of this._rows.get_children())
                row.get_last_child().reactive = true;
        }
    }

    _openBazaar() {
        try {
            Gio.Subprocess.new(BAZAAR, Gio.SubprocessFlags.STDOUT_SILENCE | Gio.SubprocessFlags.STDERR_SILENCE);
        } catch (e) {
            console.warn(`Amethystora UI: Bazaar did not start: ${e.message}`);
        }
        this.close();
    }
});

export class Opener {
    constructor(settings) {
        this._settings = settings;
    }

    enable() {
        this._settings.connectObject('changed::open-anything', () => this._apply(), this);
        this._apply();
    }

    disable() {
        this._settings?.disconnectObject(this);
        this._settings = null;
        this._unexport();
    }

    // On the bus only while the key is on, and off nothing here exists
    _apply() {
        if (!this._settings.get_boolean('open-anything')) {
            this._unexport();
        } else if (!this._dbus) {
            this._dbus = Gio.DBusExportedObject.wrapJSObject(INTERFACE, this);
            this._dbus.export(Gio.DBus.session, PATH);
        }
    }

    _unexport() {
        this._dbus?.unexport();
        this._dbus = null;
        this._dialog?.close();
        this._dialog = null;
    }

    // org.amethystora.Tools.Opener.Show: only ever handed to the helper, as a path
    Show(path) {
        if (!this._dbus || typeof path !== 'string' || !path.startsWith('/') || path.length > LONGEST_PATH || path.includes('\0'))
            return;
        this._dialog?.close();
        const dialog = new OpenerDialog(path);
        dialog.connect('destroy', () => {
            if (this._dialog === dialog)
                this._dialog = null;
        });
        this._dialog = dialog;
        dialog.open();
    }

    // For the checklist's look at what was shown
    get state() {
        return {exported: !!this._dbus, dialog: !!this._dialog, answer: this._dialog?.answer ?? null};
    }
}
