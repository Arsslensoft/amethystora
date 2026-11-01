// Bazaar in the menu of an app's icon (right-click or long press, in the app grid, its folders, the dash, the search
// results, and any dock that uses GNOME's app menu). For a Flatpak app, the details item opens the app's page in
// Bazaar rather than GNOME Software, and an Uninstall item below it removes the app from there: after asking, and
// whether to keep its data, it runs flatpak as the person, greys the app's icons while it works, and says how it
// went. Every other app keeps GNOME's own details item, which opens GNOME Software.

import Cairo from 'cairo';
import Clutter from 'gi://Clutter';
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import GObject from 'gi://GObject';
import Graphene from 'gi://Graphene';
import Shell from 'gi://Shell';
import St from 'gi://St';

import {gettext as _} from 'resource:///org/gnome/shell/extensions/extension.js';
import * as AppDisplay from 'resource:///org/gnome/shell/ui/appDisplay.js';
import * as AppMenu from 'resource:///org/gnome/shell/ui/appMenu.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import * as MessageTray from 'resource:///org/gnome/shell/ui/messageTray.js';
import * as PopupMenu from 'resource:///org/gnome/shell/ui/popupMenu.js';

import {askToUninstall, explainTwoCopies} from './bazaar/dialog.js';

Gio._promisify(Gio.Subprocess.prototype, 'wait_async');

const BAZAAR = 'io.github.kolunmi.Bazaar';
// What Flatpak accepts as an app's ID; anything else, a leading dash above all, is never handed to it
const APP_ID = /^[A-Za-z_][A-Za-z0-9_-]*(\.[A-Za-z_][A-Za-z0-9_-]*)+$/;
// The ring over an icon while its app is removed: one turn in 1.2 seconds, whatever the shell's animation speed
const TURN = 1224;
// What GNOME's app menu has that this changes, given back as they were
const WRAPPED = ['open', 'setApp', '_updateDetailsVisibility'];

// The installations an administrator added to Flatpak's own configuration, beside the machine's and the account's:
// for each, its launchers' folder and the option that names it
const INSTALLATIONS = '/etc/flatpak/installations.d';
const INSTALLATION = /^Installation "([A-Za-z0-9_.-]+)"$/;

function extraInstallations() {
    const found = [];
    let children;
    try {
        children = Gio.File.new_for_path(INSTALLATIONS).enumerate_children('standard::name',
            Gio.FileQueryInfoFlags.NONE, null);
    } catch {
        return found;
    }
    for (const child of children) {
        if (!child.get_name().endsWith('.conf'))
            continue;
        const keys = new GLib.KeyFile();
        try {
            keys.load_from_file(GLib.build_filenamev([INSTALLATIONS, child.get_name()]), GLib.KeyFileFlags.NONE);
        } catch {
            continue;
        }
        for (const group of keys.get_groups()[0]) {
            const name = INSTALLATION.exec(group)?.[1];
            let path = null;
            try {
                path = name ? keys.get_string(group, 'Path') : null;
            } catch {}
            if (path?.startsWith('/'))
                found.push([GLib.build_filenamev([path, 'exports/share/applications/']), `--installation=${name}`]);
        }
    }
    return found;
}

// The folders where Flatpak exports the launchers of the apps it installs, for the whole machine, for the account and
// for the extra installations, each with the option that names its installation. Any program of the account can
// write a launcher elsewhere in the account's own folders, with whatever X-Flatpak line it likes, so only these are
// believed.
function exportFolders() {
    return [
        ['/var/lib/flatpak/exports/share/applications/', '--system'],
        [GLib.build_filenamev([GLib.get_user_data_dir(), 'flatpak/exports/share/applications/']), '--user'],
        ...extraInstallations(),
    ];
}

// The app as Flatpak knows it: the ID Flatpak wrote into its launcher (or, for a launcher without it, the launcher's
// name) and its installation's option. Null for an app whose launcher is not one Flatpak exported.
function flatpakOf(app) {
    const info = app?.get_app_info();
    const file = info?.get_filename() ?? '';
    const where = exportFolders().find(([folder]) => file.startsWith(folder));
    if (!where)
        return null;
    const id = info.get_string('X-Flatpak')?.trim() || app.get_id().replace(/\.desktop$/, '');
    return APP_ID.test(id) ? {id, installation: where[1]} : null;
}

// (the app grid's What can it reach? asks it too)
export function flatpakId(app) {
    return flatpakOf(app)?.id ?? null;
}

function bazaarApp() {
    return Shell.AppSystem.get_default().lookup_app(`${BAZAAR}.desktop`);
}

// flatpak as the person, nothing it prints kept: whether it ended well
async function runFlatpak(argv) {
    try {
        const process = Gio.Subprocess.new(['flatpak', ...argv],
            Gio.SubprocessFlags.STDOUT_SILENCE | Gio.SubprocessFlags.STDERR_SILENCE);
        await process.wait_async(null);
        return process.get_successful();
    } catch {
        return false;
    }
}

// A ring turning over an icon's picture
const Spinner = GObject.registerClass(
class Spinner extends St.DrawingArea {
    _init(size) {
        super._init({
            width: size,
            height: size,
            x_align: Clutter.ActorAlign.CENTER,
            y_align: Clutter.ActorAlign.CENTER,
            pivot_point: new Graphene.Point({x: 0.5, y: 0.5}),
        });
    }

    vfunc_repaint() {
        const cr = this.get_context();
        const [width, height] = this.get_surface_size();
        const line = Math.max(4, width / 10);
        const radius = (Math.min(width, height) - line) / 2;
        cr.setLineWidth(line);
        cr.setLineCap(Cairo.LineCap.ROUND);
        cr.setSourceRGBA(1, 1, 1, 0.2);
        cr.arc(width / 2, height / 2, radius, 0, 2 * Math.PI);
        cr.stroke();
        cr.setSourceRGBA(1, 1, 1, 0.9);
        cr.arc(width / 2, height / 2, radius, -Math.PI / 2, Math.PI);
        cr.stroke();
        cr.$dispose();
    }

    // A timeline of its own rather than an easing, which the tweaks' speed-up would shorten; still while GNOME's
    // animations are off
    spin() {
        if (!St.Settings.get().enable_animations)
            return;
        this._turning = new Clutter.Timeline({actor: this, duration: TURN, repeat_count: -1});
        this._turning.connect('new-frame', timeline => {
            this.rotation_angle_z = 360 * timeline.get_progress();
        });
        this.connect('destroy', () => this._turning.stop());
        this._turning.start();
    }
});

// The app's icons while it is being removed. An icon the overview rebuilds in the meantime is left to it.
class Greyed {
    constructor(icons) {
        this._looks = icons.map(icon => {
            const look = {icon, reactive: icon.reactive, canFocus: icon.can_focus, picture: icon.icon?.icon ?? null};
            icon.connectObject('destroy', () => (look.icon = null), this);
            icon.reactive = false;
            icon.can_focus = false;
            const picture = look.picture;
            if (picture) {
                picture.connectObject('destroy', () => (look.picture = null), this);
                look.opacity = picture.opacity;
                picture.opacity = 180;
                picture.add_effect_with_name('amethystora-bazaar-grey', new Clutter.DesaturateEffect({factor: 1}));
                look.spinner = new Spinner(picture.width > 0 ? Math.round(picture.width / 2) : 32);
                picture.add_child(look.spinner);
                look.spinner.spin();
            }
            return look;
        });
    }

    // Their look and their clicks back
    restore() {
        for (const look of this._looks) {
            const {icon, picture} = look;
            if (picture) {
                look.spinner.destroy();
                picture.remove_effect_by_name('amethystora-bazaar-grey');
                picture.opacity = look.opacity;
                picture.disconnectObject(this);
            }
            look.picture = null;
            if (icon) {
                icon.reactive = look.reactive;
                icon.can_focus = look.canFocus;
            }
        }
    }

    // Gone with the app, until the overview rebuilds itself without it
    hide() {
        for (const look of this._looks.splice(0)) {
            look.icon?.hide();
            look.icon?.disconnectObject(this);
        }
    }

    forget() {
        for (const look of this._looks.splice(0))
            look.icon?.disconnectObject(this);
    }
}

export class Bazaar {
    constructor(ext) {
        this._ext = ext;
    }

    enable() {
        const proto = AppMenu.AppMenu.prototype;
        this._proto = proto;
        this._descriptors = new Map(WRAPPED.map(name => [name, Object.getOwnPropertyDescriptor(proto, name)]));
        this._menus = new Set();
        this._dialogs = new Set();
        this._busy = new Set();

        const module = this;
        const original = Object.fromEntries(WRAPPED.map(name => [name, proto[name]]));
        // Each menu gets its items the first time it opens, so menus made before this was turned on get them too
        proto.open = function (...args) {
            module._prepare(this);
            return original.open.apply(this, args);
        };
        proto.setApp = function (...args) {
            const result = original.setApp.apply(this, args);
            module._refresh(this);
            return result;
        };
        // GNOME's own answer whenever the installed apps change, then this module's for a Flatpak
        proto._updateDetailsVisibility = function (...args) {
            const result = original._updateDetailsVisibility.apply(this, args);
            module._refresh(this);
            return result;
        };
        this._original = original;
    }

    disable() {
        if (!this._proto)
            return;
        for (const [name, descriptor] of this._descriptors) {
            if (descriptor)
                Object.defineProperty(this._proto, name, descriptor);
            else
                delete this._proto[name];
        }
        for (const menu of this._menus)
            this._forget(menu, true);
        this._menus = null;
        for (const dialog of this._dialogs)
            dialog.close();
        this._dialogs = null;
        // An uninstall already running goes on; its icons get their look back now
        for (const job of this._busy) {
            job.restore();
            job.forget();
        }
        this._busy = null;
        this._proto = null;
        this._original = null;
    }

    _prepare(menu) {
        if (!this._menus || this._menus.has(menu) || !menu._detailsItem)
            return;
        const details = new PopupMenu.PopupMenuItem(menu._detailsItem.label.text);
        details.connect('activate', (item, event) => this._openInBazaar(menu, event));
        const uninstall = new PopupMenu.PopupMenuItem(_('Uninstall'));
        uninstall.connect('activate', () => this._uninstall(menu._app).catch(logError));
        // Right below GNOME's details item, which stays where it is for every other app
        const index = menu._getMenuItems().indexOf(menu._detailsItem);
        if (index < 0) {
            menu.addMenuItem(details);
            menu.addMenuItem(uninstall);
        } else {
            menu.addMenuItem(details, index + 1);
            menu.addMenuItem(uninstall, index + 2);
        }
        menu._amethystoraBazaar = {details, uninstall};
        menu.connectObject('destroy', () => this._forget(menu, false), this);
        this._menus.add(menu);
        this._refresh(menu);
    }

    _refresh(menu) {
        const items = menu._amethystoraBazaar;
        if (!items)
            return;
        const flatpak = flatpakId(menu._app) !== null;
        const bazaar = bazaarApp() !== null;
        if (flatpak)
            menu._detailsItem.visible = false;
        else
            this._original._updateDetailsVisibility.call(menu);
        items.details.visible = flatpak && bazaar;
        items.uninstall.visible = flatpak && bazaar;
    }

    // Takes the items away, and lets GNOME say again whether its own details item shows
    _forget(menu, restore) {
        menu.disconnectObject(this);
        const items = menu._amethystoraBazaar;
        delete menu._amethystoraBazaar;
        this._menus?.delete(menu);
        if (!restore || !items)
            return;
        items.details.destroy();
        items.uninstall.destroy();
        this._original._updateDetailsVisibility.call(menu);
    }

    // The app's page in Bazaar, through Bazaar's own launcher and the shell's launch context, so that an open Bazaar
    // comes to the front
    _openInBazaar(menu, event) {
        const id = flatpakId(menu._app);
        const info = bazaarApp()?.get_app_info();
        if (!id || !info)
            return;
        info.launch_uris([`appstream:${id}`], global.create_app_launch_context(event?.get_time?.() ?? 0, -1));
        Main.overview.hide();
    }

    async _uninstall(app) {
        const flatpak = flatpakOf(app);
        if (!flatpak)
            return;
        const {id, installation} = flatpak;
        const name = app.get_name() || id;
        // An extra installation's app is looked for there alone, as the machine's copy; any other in both the
        // account's and the machine's
        const extra = installation.startsWith('--installation=');
        const [user, system] = extra
            ? [false, await runFlatpak(['info', installation, id])]
            : await Promise.all([runFlatpak(['info', '--user', id]), runFlatpak(['info', '--system', id])]);
        if (!this._menus)
            return;
        if (!user && !system) {
            this._notify(_('Nothing to uninstall'), _('No Flatpak of %s was found.').format(name));
            return;
        }
        if (user && system) {
            this._track(explainTwoCopies(name));
            return;
        }

        const {dialog, answer} = askToUninstall(name, id);
        this._track(dialog);
        const deleteData = await answer;
        if (deleteData === null || !this._menus)
            return;

        const job = this._grey(app);
        this._busy.add(job);
        const argv = ['uninstall', user ? '--user' : (extra ? installation : '--system'), '-y', id];
        if (deleteData)
            argv.push('--delete-data');
        const done = await runFlatpak(argv);
        // Turned off meanwhile: the icons already have their look back
        const running = this._busy?.delete(job) ?? false;
        if (running)
            job.restore();
        if (done && running)
            job.hide();
        else
            job.forget();
        if (done) {
            this._notify(_('%s is uninstalled').format(name),
                deleteData ? _('Its data was deleted too.') : _('Its data was kept.'));
        } else {
            this._notify(_('%s could not be uninstalled').format(name), _('Try uninstalling it in Bazaar.'));
        }
    }

    _track(dialog) {
        this._dialogs.add(dialog);
        dialog.connect('closed', () => this._dialogs?.delete(dialog));
    }

    // Every icon of the app in the overview, greyed with a ring over its picture, taking no clicks, drags or focus
    _grey(app) {
        const id = app.get_id();
        const bare = id.replace(/.desktop$/, '');
        const icons = [];
        const walk = actor => {
            if (actor instanceof AppDisplay.AppIcon) {
                const appId = actor.app?.get_id();
                if (appId === id || appId === bare)
                    icons.push(actor);
                return;
            }
            for (const child of actor.get_children())
                walk(child);
        };
        walk(Main.layoutManager.overviewGroup);
        return new Greyed(icons);
    }

    // Said by Bazaar, in passing: it does not stay in the list
    _notify(title, body) {
        const source = new MessageTray.Source({
            title: bazaarApp()?.get_name() ?? 'Bazaar',
            iconName: BAZAAR,
        });
        Main.messageTray.add(source);
        source.addNotification(new MessageTray.Notification({source, title, body, isTransient: true}));
    }
}
