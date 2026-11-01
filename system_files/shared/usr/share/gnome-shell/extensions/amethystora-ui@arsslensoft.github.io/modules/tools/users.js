// What is using this? The account's own programs that hold a file, a folder or a drive, each with Quit: from Files'
// Scripts menu (through /usr/libexec/amethystora-what-uses show, which asks here over D-Bus), from GNOME's own "volume
// busy" dialog, which gains a line that lists them in it, and from the dock's notice when a drive will not eject.
//
// What a caller passes is only ever paths: the list is always read here, by running the helper as the account, and it
// names only the account's own processes. A program is shown as the app it belongs to (the nearest of its parents
// that has one: the terminal a shell runs in), else by the name it gives itself, as text.

import Clutter from 'gi://Clutter';
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import GObject from 'gi://GObject';
import Pango from 'gi://Pango';
import Shell from 'gi://Shell';
import St from 'gi://St';

import {gettext as _, ngettext} from 'resource:///org/gnome/shell/extensions/extension.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import * as ModalDialog from 'resource:///org/gnome/shell/ui/modalDialog.js';

import {run} from './run.js';

const HELPER = '/usr/libexec/amethystora-what-uses';
const PATH = '/org/amethystora/Tools';
const INTERFACE = `
<node>
  <interface name="org.amethystora.Tools">
    <method name="ShowUsers">
      <arg type="as" direction="in" name="paths"/>
    </method>
  </interface>
</node>`;
const MOST_PATHS = 64;
const LONGEST_PATH = 4096;
// After a Quit, the list is read again once, this long after
const AGAIN = 1200;

// A name a program gave itself, as text a person can read: no control characters, at most 40 characters
function plainName(text) {
    // eslint-disable-next-line no-control-regex
    const clean = String(text ?? '').replace(/[\u0000-\u001f\u007f-\u009f‎‏‪-‮⁦-⁩]/g, '').trim();
    return clean.slice(0, 40) || '?';
}

function basename(path) {
    return plainName(GLib.path_get_basename(path));
}

// The paths of the removable drives mounted now: what GNOME's busy dialog is about
function removableRoots() {
    const roots = [];
    for (const mount of Gio.VolumeMonitor.get().get_mounts()) {
        if (!mount.can_unmount() && !mount.can_eject())
            continue;
        const path = mount.get_root()?.get_path();
        if (path && path !== '/')
            roots.push(path);
    }
    return roots;
}

// The rows of the list: one per app (or per program without one), with what it holds
async function readHolders(paths, cancellable) {
    const {ok, stdout} = await run([HELPER, 'list', '--json', '--', ...paths], {timeout: 15000, cancellable});
    if (!ok)
        return null;
    let data;
    try {
        data = JSON.parse(stdout);
    } catch {
        return null;
    }
    const tracker = Shell.WindowTracker.get_default();
    const rows = new Map();
    for (const holder of Array.isArray(data?.holders) ? data.holders : []) {
        if (!Number.isInteger(holder?.pid) || !Number.isInteger(holder?.start))
            continue;
        let app = null;
        for (const pid of [holder.pid, ...(Array.isArray(holder.ancestors) ? holder.ancestors : [])]) {
            if (!Number.isInteger(pid))
                continue;
            app = tracker.get_app_from_pid(pid);
            if (app)
                break;
        }
        const key = app ? `app:${app.get_id()}` : `pid:${holder.pid}`;
        const row = rows.get(key) ?? {app, name: app ? app.get_name() : plainName(holder.name), processes: [], reasons: [], more: 0,
            essential: false};
        row.processes.push({pid: holder.pid, start: holder.start});
        row.essential ||= holder.essential === true;
        for (const reason of Array.isArray(holder.reasons) ? holder.reasons : []) {
            if (typeof reason?.path === 'string' && ['cwd', 'file', 'exe', 'map'].includes(reason.kind))
                row.reasons.push(reason);
        }
        row.more += Number.isInteger(holder.more) ? holder.more : 0;
        rows.set(key, row);
    }
    return [...rows.values()];
}

// What a row holds, in a few words: the first reason, and how many more
function whatItHolds(row) {
    const [first] = row.reasons;
    if (!first)
        return '';
    let words;
    switch (first.kind) {
    case 'cwd': words = _('its folder is open here'); break;
    case 'exe': words = _('it runs from here'); break;
    case 'map': words = _('%s is in use').format(basename(first.path)); break;
    default: words = _('%s is open').format(basename(first.path));
    }
    const more = row.reasons.length - 1 + row.more;
    return more > 0 ? ngettext('%s, and %d more', '%s, and %d more', more).format(words, more) : words;
}

// The list itself, which a dialog of its own or GNOME's busy dialog shows
export const UsersList = GObject.registerClass(
class UsersList extends St.BoxLayout {
    _init(paths) {
        super._init({style_class: 'amethystora-tools-users', orientation: Clutter.Orientation.VERTICAL, x_expand: true});
        this._paths = paths;
        this._cancellable = new Gio.Cancellable();
        this._rowsBox = new St.BoxLayout({style_class: 'amethystora-tools-users-rows', orientation: Clutter.Orientation.VERTICAL});
        this.add_child(this._rowsBox);
        this._status = new St.Label({style_class: 'amethystora-tools-users-status', text: _('Looking…')});
        this.add_child(this._status);
        this.add_child(new St.Label({style_class: 'amethystora-tools-users-footer', text: _('Only your own programs are listed.')}));
        this.rows = null;
        this.connect('destroy', () => {
            this._cancellable.cancel();
            if (this._again)
                GLib.source_remove(this._again);
            this._again = 0;
        });
        this.refresh();
    }

    refresh() {
        readHolders(this._paths, this._cancellable).then(rows => {
            if (this._cancellable.is_cancelled())
                return;
            this._show(rows);
        }).catch(e => {
            if (!e.matches?.(Gio.IOErrorEnum, Gio.IOErrorEnum.CANCELLED))
                console.warn(`Amethystora UI: what is using it could not be read: ${e.message}`);
        });
    }

    _show(rows) {
        this.rows = rows;
        this._rowsBox.destroy_all_children();
        if (rows === null) {
            this._status.text = _('What is using it could not be read.');
            this._status.show();
            return;
        }
        if (rows.length === 0) {
            this._status.text = _('Nothing of yours is using it.');
            this._status.show();
            return;
        }
        this._status.hide();
        for (const row of rows)
            this._rowsBox.add_child(this._row(row));
    }

    _row(row) {
        const box = new St.BoxLayout({style_class: 'amethystora-tools-users-row'});
        const tile = new St.Bin({style_class: 'amethystora-tools-users-icon', y_align: Clutter.ActorAlign.CENTER});
        tile.set_child(row.app ? row.app.create_icon_texture(20)
            : new St.Icon({icon_name: 'utilities-terminal-symbolic', icon_size: 16}));
        box.add_child(tile);
        const words = new St.BoxLayout({orientation: Clutter.Orientation.VERTICAL, x_expand: true, y_align: Clutter.ActorAlign.CENTER});
        const name = new St.Label({style_class: 'amethystora-tools-users-name', text: row.name});
        const detail = new St.Label({style_class: 'amethystora-tools-users-detail', text: whatItHolds(row)});
        for (const label of [name, detail]) {
            label.clutter_text.use_markup = false;
            label.clutter_text.ellipsize = Pango.EllipsizeMode.END;
        }
        words.add_child(name);
        words.add_child(detail);
        box.add_child(words);
        if (!row.essential) {
            const quit = new St.Button({style_class: 'amethystora-tools-users-quit', label: _('Quit'), can_focus: true,
                y_align: Clutter.ActorAlign.CENTER});
            quit.accessible_name = _('Quit %s').format(row.name);
            quit.connect('clicked', () => {
                quit.reactive = false;
                this._quit(row);
            });
            box.add_child(quit);
        }
        return box;
    }

    // An app is asked to quit as GNOME's own Quit does; a program without one is sent SIGTERM by the helper, which
    // checks it is still the one listed
    _quit(row) {
        if (row.app) {
            row.app.request_quit();
        } else {
            for (const {pid, start} of row.processes)
                run([HELPER, 'quit', `${pid}`, `${start}`], {timeout: 5000}).catch(() => {});
        }
        if (this._again)
            GLib.source_remove(this._again);
        this._again = GLib.timeout_add(GLib.PRIORITY_DEFAULT, AGAIN, () => {
            this._again = 0;
            this.refresh();
            return GLib.SOURCE_REMOVE;
        });
    }
});

export const UsersDialog = GObject.registerClass(
class UsersDialog extends ModalDialog.ModalDialog {
    _init(paths, name) {
        super._init({styleClass: 'amethystora-tools-users-dialog', destroyOnClose: true});
        const title = new St.Label({style_class: 'amethystora-tools-users-title', text: _('Using %s').format(name)});
        title.clutter_text.use_markup = false;
        title.clutter_text.ellipsize = Pango.EllipsizeMode.MIDDLE;
        this.contentLayout.add_child(title);
        this.list = new UsersList(paths);
        this.contentLayout.add_child(this.list);
        this.setButtons([{label: _('Close'), action: () => this.close(), key: Clutter.KEY_Escape, default: true}]);
    }
});

export class Users {
    enable() {
        this._dbus = Gio.DBusExportedObject.wrapJSObject(INTERFACE, this);
        this._dbus.export(Gio.DBus.session, PATH);
        // GNOME's busy dialog: the line is added as it opens
        const users = this;
        const open = ModalDialog.ModalDialog.prototype.open;
        this._open = open;
        ModalDialog.ModalDialog.prototype.open = function (...args) {
            const result = open.apply(this, args);
            try {
                users._busyDialogOpened(this);
            } catch (e) {
                console.warn(`Amethystora UI: GNOME's busy dialog was not given its line: ${e.message}`);
            }
            return result;
        };
    }

    disable() {
        if (this._open)
            ModalDialog.ModalDialog.prototype.open = this._open;
        this._open = null;
        this._dbus?.unexport();
        this._dbus = null;
        this._dialog?.close();
        this._dialog = null;
    }

    // org.amethystora.Tools.ShowUsers: the paths are checked, and only ever handed to the helper as paths
    ShowUsers(paths) {
        if (!Array.isArray(paths) || paths.length === 0 || paths.length > MOST_PATHS)
            return;
        const wanted = paths.filter(path => typeof path === 'string' && path.startsWith('/') &&
            path.length <= LONGEST_PATH && !path.includes('\0'));
        if (wanted.length === 0)
            return;
        this.show(wanted, wanted.length === 1 ? basename(wanted[0]) : _('the files picked'));
    }

    show(paths, name) {
        this._dialog?.close();
        const dialog = new UsersDialog(paths, name);
        dialog.connect('destroy', () => {
            if (this._dialog === dialog)
                this._dialog = null;
        });
        this._dialog = dialog;
        dialog.open();
    }

    // A drive that will not eject: its notice offers this
    showForMount(mount) {
        const path = mount?.get_root?.()?.get_path?.();
        if (path)
            this.show([path], plainName(mount.get_name()));
    }

    _busyDialogOpened(dialog) {
        const layout = dialog.dialogLayout?._dialog;
        if (!layout?.has_style_class_name?.('processes-dialog') || dialog._amethystoraUsers)
            return;
        const line = new St.Button({style_class: 'amethystora-tools-users-show button', label: _('Show what is using it'),
            can_focus: true, x_align: Clutter.ActorAlign.CENTER});
        dialog._amethystoraUsers = line;
        line.connect('clicked', () => {
            const roots = removableRoots();
            line.destroy();
            dialog._amethystoraUsers = new UsersList(roots);
            dialog.contentLayout.add_child(dialog._amethystoraUsers);
        });
        dialog.contentLayout.add_child(line);
    }

    // For the suites
    get state() {
        return {
            exported: !!this._dbus,
            dialog: !!this._dialog,
            rows: this._dialog?.list?.rows?.map(row => ({name: row.name, detail: whatItHolds(row), essential: row.essential})) ?? null,
        };
    }
}
