// Your backup disk, recognized: plugging in the disk the account's backup goes to asks "Back up now?", starts the
// account's own backup (amethystora-backup.service, as Backups' Back up now does), and once it is done puts the disk
// away and says it is safe to unplug. The disk is known by the place the backup was set up on: a local restic
// repository (RESTIC_REPOSITORY in ~/.config/amethystora/backup.env, that one line and nothing else of the file) inside
// the new mount. Nothing runs but on a mount being added (Gio's volume monitor), and the unit's own signals while a
// backup it started runs. The key `ask` turns it off.

import Gio from 'gi://Gio';
import GLib from 'gi://GLib';

import {gettext as _, ngettext} from 'resource:///org/gnome/shell/extensions/extension.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import * as MessageTray from 'resource:///org/gnome/shell/ui/messageTray.js';

Gio._promisify(Gio.File.prototype, 'load_contents_async');
Gio._promisify(Gio.Mount.prototype, 'unmount_with_operation');
Gio._promisify(Gio.Drive.prototype, 'eject_with_operation');

const SCHEMA = 'org.gnome.shell.extensions.amethystora-ui.backupdisk';
const UNIT = 'amethystora-backup.service';
const SYSTEMD = 'org.freedesktop.systemd1';
const MANAGER = '/org/freedesktop/systemd1';
// The settings file is a few lines; anything larger is not read
const LARGEST = 16384;

// A backup this started, kept across a screen lock (which stops the extension): the disk's place, so that its end is
// still said, and the disk put away, after the unlock
let job = null;

function configFile() {
    return Gio.File.new_for_path(GLib.build_filenamev([GLib.get_user_config_dir(), 'amethystora', 'backup.env']));
}

// The repository's folder, when it is a local one: "/path" or "local:/path", quoted or not
export function localRepository(text) {
    for (const line of text.split('\n')) {
        const found = /^\s*(?:export\s+)?RESTIC_REPOSITORY=(.*)$/.exec(line);
        if (!found)
            continue;
        let value = found[1].trim();
        if ((value.startsWith('"') && value.endsWith('"')) || (value.startsWith("'") && value.endsWith("'")))
            value = value.slice(1, -1);
        value = value.replace(/^local:/, '');
        if (!value.startsWith('/') || value.includes('/../') || value.endsWith('/..'))
            return null;
        return same(value.replace(/\/+$/, '') || '/');
    }
    return null;
}

// /var/run is /run on this system
function same(path) {
    return path.replace(/^\/var\/run\//, '/run/');
}

export function inside(repository, root) {
    const top = same(root).replace(/\/+$/, '');
    return Boolean(repository && top && top !== '' && (repository === top || repository.startsWith(`${top}/`)));
}

// "Last backup six days ago" from the stamp amethystora-backup writes after each good run
function lastBackup() {
    const stamp = GLib.build_filenamev([GLib.get_user_state_dir(), 'amethystora', 'backup-last-success']);
    let when = 0;
    try {
        const [, bytes] = GLib.file_get_contents(stamp);
        when = parseInt(new TextDecoder().decode(bytes).trim(), 10);
    } catch {}
    if (!Number.isFinite(when) || when <= 0)
        return _('Nothing is backed up on it yet.');
    const days = Math.floor((GLib.get_real_time() / 1e6 - when) / 86400);
    if (days <= 0)
        return _('Last backed up today.');
    if (days === 1)
        return _('Last backed up yesterday.');
    return ngettext('Last backed up %d day ago.', 'Last backed up %d days ago.', days).format(days);
}

export class BackupDisk {
    constructor(ext) {
        this._ext = ext;
    }

    enable() {
        this._settings = this._ext.getSettings(SCHEMA);
        this._settings.connectObject('changed::ask', () => this._sync(), this);
        this._sync();
        // A backup started before the screen was locked: follow it again
        if (job)
            this._follow().catch(e => console.error(`Amethystora UI: backupdisk: ${e.message}`));
    }

    disable() {
        this._settings?.disconnectObject(this);
        this._unwatch();
        this._stopFollowing();
        this._source?.destroy();
        this._source = null;
        // Disposed now rather than by the collector (dock.js says why)
        this._settings?.run_dispose();
        this._settings = null;
    }

    _sync() {
        if (this._settings.get_boolean('ask'))
            this._watch();
        else
            this._unwatch();
    }

    _watch() {
        if (this._monitor)
            return;
        this._monitor = Gio.VolumeMonitor.get();
        this._monitor.connectObject('mount-added', (monitor, mount) => {
            this._added(mount).catch(e => console.error(`Amethystora UI: backupdisk: ${e.message}`));
        }, this);
    }

    _unwatch() {
        this._monitor?.disconnectObject(this);
        this._monitor = null;
    }

    async _added(mount) {
        const root = mount?.get_root?.()?.get_path();
        if (!root || job)
            return;
        let text;
        try {
            const [bytes] = await configFile().load_contents_async(null);
            if (bytes.length > LARGEST)
                return;
            text = new TextDecoder().decode(bytes);
        } catch {
            return;
        }
        if (!this._settings || !inside(localRepository(text), root))
            return;
        const state = await this._unitState();
        if (!this._settings || state === 'active' || state === 'activating')
            return;
        this._ask(root);
    }

    _notify(title, body, actions = [], resident = false) {
        if (!this._source) {
            this._source = new MessageTray.Source({title: _('Backups'), iconName: 'drive-harddisk-symbolic'});
            this._source.connect('destroy', () => (this._source = null));
            Main.messageTray.add(this._source);
        }
        this._shown?.destroy();
        const notification = new MessageTray.Notification({source: this._source, title, body, resident});
        for (const [label, callback] of actions)
            notification.addAction(label, callback);
        notification.connect('destroy', () => {
            if (this._shown === notification)
                this._shown = null;
        });
        this._shown = notification;
        this._source.addNotification(notification);
        return notification;
    }

    _ask(root) {
        this._notify(_('Your backup disk'), lastBackup(), [
            [_('Not now'), () => {}],
            [_('Back up now'), () => this._backUp(root).catch(e => console.error(`Amethystora UI: backupdisk: ${e.message}`))],
        ]);
    }

    // --- The account's backup, through its own user manager ------------------------------------------------------

    async _manager(method, args, type) {
        return await Gio.DBus.session.call(SYSTEMD, MANAGER, 'org.freedesktop.systemd1.Manager', method, args,
            type ? new GLib.VariantType(type) : null, Gio.DBusCallFlags.NONE, -1, null);
    }

    async _unitPath() {
        const reply = await this._manager('LoadUnit', new GLib.Variant('(s)', [UNIT]), '(o)');
        return reply.deepUnpack()[0];
    }

    async _property(path, iface, name) {
        const reply = await Gio.DBus.session.call(SYSTEMD, path, 'org.freedesktop.DBus.Properties', 'Get',
            new GLib.Variant('(ss)', [iface, name]), new GLib.VariantType('(v)'), Gio.DBusCallFlags.NONE, -1, null);
        return reply.deepUnpack()[0].deepUnpack();
    }

    async _unitState() {
        try {
            return await this._property(await this._unitPath(), 'org.freedesktop.systemd1.Unit', 'ActiveState');
        } catch {
            return 'unknown';
        }
    }

    async _backUp(root) {
        job = {root, started: false};
        try {
            await this._manager('StartUnit', new GLib.Variant('(ss)', [UNIT, 'replace']), '(o)');
        } catch (e) {
            job = null;
            this._notify(_('The backup did not start'), _('Open Backups to see why.'), [[_('Open Backups'), () => this._openBackups()]]);
            console.error(`Amethystora UI: backupdisk: starting the backup: ${e.message}`);
            return;
        }
        job.started = true;
        this._notify(_('Backing up to your backup disk'), _('You will be told when it is safe to unplug.'), [], true);
        await this._follow();
    }

    // The unit's own signals: systemd says each change of its state, which ends this when the run is over
    async _follow() {
        if (!job || this._watching)
            return;
        const path = await this._unitPath();
        if (!this._settings || !job)
            return;
        try {
            await this._manager('Subscribe', null, null);
        } catch {}
        this._watching = Gio.DBus.session.signal_subscribe(SYSTEMD, 'org.freedesktop.DBus.Properties', 'PropertiesChanged',
            path, null, Gio.DBusSignalFlags.NONE, () => this._check(path));
        this._check(path);
    }

    _stopFollowing() {
        if (this._watching)
            Gio.DBus.session.signal_unsubscribe(this._watching);
        this._watching = 0;
    }

    async _check(path) {
        let state;
        try {
            state = await this._property(path, 'org.freedesktop.systemd1.Unit', 'ActiveState');
        } catch {
            return;
        }
        if (!job || !this._settings || state === 'active' || state === 'activating' || state === 'reloading')
            return;
        this._stopFollowing();
        let result = 'success';
        try {
            result = await this._property(path, 'org.freedesktop.systemd1.Service', 'Result');
        } catch {}
        const {root} = job;
        job = null;
        if (state === 'failed' || result !== 'success') {
            this._notify(_('The backup did not finish'), _('The disk can be unplugged. Backups says what went wrong.'),
                [[_('Open Backups'), () => this._openBackups()]]);
            return;
        }
        await this._putAway(root);
    }

    async _putAway(root) {
        const mount = (Gio.VolumeMonitor.get().get_mounts() ?? []).find(m => m.get_root()?.get_path() === root);
        if (!mount) {
            this._notify(_('Safe to unplug'), _('Your backup is done.'));
            return;
        }
        try {
            const drive = mount.get_drive();
            if (drive?.can_eject())
                await drive.eject_with_operation(Gio.MountUnmountFlags.NONE, null, null);
            else
                await mount.unmount_with_operation(Gio.MountUnmountFlags.NONE, null, null);
            if (this._settings)
                this._notify(_('Safe to unplug'), _('Your backup is done, and the disk is put away.'));
        } catch {
            if (this._settings)
                this._notify(_('The backup is done'), _('The disk is still in use: put it away in Files before unplugging it.'));
        }
    }

    _openBackups() {
        try {
            Gio.Subprocess.new(['amethystora-backups'], Gio.SubprocessFlags.STDOUT_SILENCE | Gio.SubprocessFlags.STDERR_SILENCE);
        } catch (e) {
            console.error(`Amethystora UI: backupdisk: opening Backups: ${e.message}`);
        }
    }
}
