// The app grid as the account wants it: an app hidden from the grid and the search, an app's own name or icon
// changed, and the grid in an order (A to Z, by use, by kind). Each is a key of the appgrid schema (hidden, names,
// icons, order), applied where GNOME already decides: what the grid, its folders and the search show passes through
// Parental Controls' check, an app's name and icon come from Shell.App, and each icon's place from the grid's own
// _getItemPosition. No launcher is ever written: one in the home folder is what the security watcher reads as planted.
//
// The menu of an app's icon in the grid, its folders, the recent row and the search gains Rename…, Change icon…, Hide
// from the grid and search and, for a Flatpak app, What can it reach?, which opens Security's Apps page on that app and
// changes nothing itself. The dock's menus stay GNOME's.

import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import Shell from 'gi://Shell';
import St from 'gi://St';

import {InjectionManager, gettext as _} from 'resource:///org/gnome/shell/extensions/extension.js';
import * as AppDisplay from 'resource:///org/gnome/shell/ui/appDisplay.js';
import * as AppFavorites from 'resource:///org/gnome/shell/ui/appFavorites.js';
import * as AppMenu from 'resource:///org/gnome/shell/ui/appMenu.js';
import * as Dash from 'resource:///org/gnome/shell/ui/dash.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import * as MessageTray from 'resource:///org/gnome/shell/ui/messageTray.js';
import * as ParentalControlsManager from 'resource:///org/gnome/shell/misc/parentalControlsManager.js';
import * as PopupMenu from 'resource:///org/gnome/shell/ui/popupMenu.js';

import {flatpakId} from '../bazaar.js';
import {askIcon, askName} from './dialogs.js';

Gio._promisify(Gio.Subprocess.prototype, 'communicate_utf8_async');
Gio._promisify(Gio.Subprocess.prototype, 'wait_async');

// The renamed apps' names by launcher ID, which the forgiving search (fuzzy.js) finds them by too
export const renamed = new Map();

export const LAUNCHER = /^[A-Za-z0-9_][A-Za-z0-9._+-]{0,250}[.]desktop$/;
export const PACK = /^amethystora-[a-z][a-z-]{0,30}$/;
const PICTURE = /^picture:[0-9]{1,12}$/;
export const NAME_MAX = 64;
const ICONS = '/usr/share/icons';
const LOOK = '/usr/libexec/amethystora-app-look';
const SECURITY = 'amethystora-security';
const ORDERS = ['custom', 'name', 'use', 'kind'];

// What each kind is, by the launchers' main and additional categories (the freedesktop menu specification's). An app
// takes the first kind whose categories it names, in this order, so that a code editor that also says TextEditor is
// development's and a browser work's; the grid shows them in the order of `place`.
const KINDS = [
    {kind: 'games', place: 3, categories: ['Game']},
    {kind: 'development', place: 2, categories: ['Development', 'IDE']},
    {kind: 'media', place: 1, categories: ['AudioVideo', 'Audio', 'Video', 'Graphics', 'Photography', 'Player', 'Recorder']},
    {kind: 'work', place: 0, categories: ['Office', 'TextEditor', 'WordProcessor', 'Spreadsheet', 'Presentation', 'Calendar',
        'ContactManagement', 'Finance', 'Education', 'Science', 'Network', 'WebBrowser', 'Email', 'Chat', 'InstantMessaging']},
    {kind: 'system', place: 4, categories: ['System', 'Settings', 'Utility', 'FileManager', 'TerminalEmulator', 'Security',
        'Monitor', 'PackageManager']},
];
const OTHER = 5;

function kindOf(app) {
    let categories = [];
    try {
        categories = (app?.app_info?.get_categories() ?? '').split(';').filter(Boolean);
    } catch {}
    return KINDS.find(({categories: of}) => of.some(c => categories.includes(c)))?.place ?? OTHER;
}

// The icon pack's drawing of an app, as the packs lay themselves out (branding/icons.mjs): apps/scalable/<name>.svg
export function packFile(pack, app) {
    if (!PACK.test(pack ?? ''))
        return null;
    const icon = app?.app_info?.get_icon?.();
    const names = icon instanceof Gio.ThemedIcon ? icon.get_names() : [app?.get_id()?.replace(/[.]desktop$/, '')];
    for (const name of names) {
        if (!name || name.includes('/'))
            continue;
        const path = GLib.build_filenamev([ICONS, pack, 'apps', 'scalable', `${name}.svg`]);
        if (GLib.file_test(path, GLib.FileTest.IS_REGULAR))
            return path;
    }
    return null;
}

// The installed packs that draw this app, each with its name as its index.theme gives it
export function packsFor(app) {
    const found = [];
    let children;
    try {
        children = Gio.File.new_for_path(ICONS).enumerate_children('standard::name', Gio.FileQueryInfoFlags.NONE, null);
    } catch {
        return found;
    }
    for (const info of children) {
        const pack = info.get_name();
        const file = packFile(pack, app);
        if (!file)
            continue;
        let title = pack;
        const keys = new GLib.KeyFile();
        try {
            keys.load_from_file(GLib.build_filenamev([ICONS, pack, 'index.theme']), GLib.KeyFileFlags.NONE);
            title = keys.get_locale_string('Icon Theme', 'Name', null) || pack;
        } catch {}
        // Every pack is Amethystora's: its own word is enough under a tile ("Glass", "Neon for light")
        title = title.replace(/^Amethystora\s+/, '') || title;
        found.push({pack, file, title});
    }
    return found.sort((a, b) => a.title.localeCompare(b.title));
}

// The picture amethystora-app-look wrote for an app: picture:N is ~/.local/share/amethystora/app-icons/<ID>-N.png, a
// new N for each new picture, since St keeps a file's texture for as long as its name stays the same
function pictureFile(id, value) {
    const stamp = value.slice('picture:'.length);
    return GLib.build_filenamev([GLib.get_user_data_dir(), 'amethystora', 'app-icons',
        `${id.replace(/[.]desktop$/, '')}-${stamp}.png`]);
}

export class Looks {
    constructor(settings, appDisplay) {
        this._settings = settings;
        this._appDisplay = appDisplay;
    }

    enable() {
        this._injections = new InjectionManager();
        this._menus = new Set();
        this._dialogs = new Set();
        this._passing = 0;
        this._read();
        this._settings.connectObject(
            'changed::hidden', () => this._changed('hidden'),
            'changed::names', () => this._changed('names'),
            'changed::icons', () => this._changed('icons'),
            'changed::order', () => this._changed('order'), this);
        this._wrapFilter();
        this._wrapApps();
        this._wrapOrder();
        this._wrapMenu();
        if (this._hidden.size)
            this._parental.emit('app-filter-changed');
        // Only when there is something of the account's to draw: a lock and an unlock stop and start this
        if (this._names.size || this._icons.size)
            this._refresh();
        if (this._order !== 'custom')
            this._redisplay();
    }

    disable() {
        this._settings?.disconnectObject(this);
        for (const dialog of this._dialogs ?? [])
            dialog.close();
        this._dialogs = null;
        for (const menu of this._menus ?? []) {
            menu.disconnectObject?.(this);
            menu._amethystoraLooks?.section.destroy();
            delete menu._amethystoraLooks;
        }
        this._menus = null;
        this._source?.destroy();
        this._source = null;
        const hadHidden = this._hidden?.size > 0;
        const hadLooks = this._names?.size > 0 || this._icons?.size > 0;
        const wasSorted = this._sorted !== null && this._sorted !== undefined;
        this._sorted = null;
        this._injections?.clear();
        this._injections = null;
        renamed.clear();
        this._hidden = new Set();
        this._names = new Map();
        this._icons = new Map();
        // What GNOME shows again: every app, its own name and icon, and the account's own order
        if (hadHidden)
            this._parental?.emit('app-filter-changed');
        if (hadLooks)
            this._refresh();
        if (wasSorted)
            this._redisplay();
        this._parental = null;
        this._settings = null;
        this._appDisplay = null;
    }

    // --- The keys -------------------------------------------------------------------------------------------------

    _read() {
        this._hidden = new Set(this._settings.get_strv('hidden').filter(id => LAUNCHER.test(id)));
        const pairs = key => Object.entries(this._settings.get_value(key).deepUnpack() ?? {});
        this._names = new Map(pairs('names')
            .map(([id, name]) => [id, clean(name)])
            .filter(([id, name]) => LAUNCHER.test(id) && name));
        renamed.clear();
        for (const [id, name] of this._names)
            renamed.set(id, name);
        this._icons = new Map(pairs('icons').filter(([id, value]) => LAUNCHER.test(id) &&
            (PACK.test(value) || PICTURE.test(value))));
        this._gicons = new Map();
        const order = this._settings.get_string('order');
        this._order = ORDERS.includes(order) ? order : 'custom';
    }

    _changed(key) {
        const before = {hidden: this._hidden, order: this._order};
        this._read();
        if (key === 'hidden') {
            const same = before.hidden.size === this._hidden.size && [...this._hidden].every(id => before.hidden.has(id));
            if (!same)
                this._parental.emit('app-filter-changed');
        } else if (key === 'names' || key === 'icons') {
            this._refresh();
        }
        if (this._order !== 'custom' || before.order !== this._order)
            this._redisplay();
    }

    _write(key, change) {
        const value = this._settings.get_value(key).deepUnpack() ?? {};
        change(value);
        this._settings.set_value(key, new GLib.Variant('a{ss}', value));
    }

    hide(id, hidden) {
        const list = this._settings.get_strv('hidden').filter(other => other !== id);
        if (hidden)
            list.push(id);
        this._settings.set_strv('hidden', list);
    }

    // --- Hidden apps -----------------------------------------------------------------------------------------------

    // Parental Controls' check is what the grid, its folders, the search, the forgiving search and the recent rows ask
    // about every app; it is asked on its one instance. The dock's pinned apps are not the grid: while the favourites
    // read their list, add one or say whether Pin or Unpin shows, a hidden app passes, so that it stays pinned.
    _wrapFilter() {
        const looks = this;
        this._parental = ParentalControlsManager.getDefault();
        this._injections.overrideMethod(this._parental, 'shouldShowApp', original => function (appInfo) {
            if (!original.call(this, appInfo))
                return false;
            if (looks._passing > 0 || !looks._hidden?.size)
                return true;
            let id = null;
            try {
                id = appInfo?.get_id();
            } catch {}
            return !looks._hidden.has(id);
        });
        const passing = original => function (...args) {
            looks._passing++;
            try {
                return original.apply(this, args);
            } finally {
                looks._passing--;
            }
        };
        const favorites = AppFavorites.getAppFavorites();
        for (const name of ['reload', '_addFavorite']) {
            if (typeof favorites[name] === 'function')
                this._injections.overrideMethod(favorites, name, passing);
        }
        if (typeof AppMenu.AppMenu.prototype._updateFavoriteItem === 'function')
            this._injections.overrideMethod(AppMenu.AppMenu.prototype, '_updateFavoriteItem', passing);
    }

    // --- Names and icons ------------------------------------------------------------------------------------------

    _wrapApps() {
        const looks = this;
        const proto = Shell.App.prototype;
        this._injections.overrideMethod(proto, 'get_name', original => function () {
            return looks._names?.get(this.get_id()) ?? original.call(this);
        });
        this._injections.overrideMethod(proto, 'get_icon', original => function () {
            return looks._gicon(this) ?? original.call(this);
        });
        this._injections.overrideMethod(proto, 'create_icon_texture', original => {
            looks._ownTexture = (app, size) => original.call(app, size);
            return function (size) {
                const texture = original.call(this, size);
                const gicon = looks._gicon(this);
                if (gicon && texture instanceof St.Icon)
                    texture.gicon = gicon;
                return texture;
            };
        });
    }

    // The icon chosen for an app, made once per change of the key; null for its own, or when the file is gone
    _gicon(app) {
        const id = app?.get_id();
        const value = id ? this._icons?.get(id) : undefined;
        if (!value)
            return null;
        if (this._gicons.has(id))
            return this._gicons.get(id);
        let path = null;
        if (PACK.test(value))
            path = packFile(value, app);
        else if (PICTURE.test(value))
            path = pictureFile(id, value);
        const gicon = path && GLib.file_test(path, GLib.FileTest.IS_REGULAR)
            ? Gio.FileIcon.new(Gio.File.new_for_path(path)) : null;
        this._gicons.set(id, gicon);
        return gicon;
    }

    ownIcon(app, size) {
        return this._ownTexture ? this._ownTexture(app, size) : app.create_icon_texture(size);
    }

    // The icons already drawn, with the names and pictures as they are now: the grid's, its folders', the recent row's,
    // the search's and the dock's. Window actors are not looked into.
    _refresh() {
        const walk = actor => {
            if (actor === global.window_group)
                return;
            if (actor instanceof AppDisplay.AppIcon && actor.app) {
                const name = actor.app.get_name();
                if (actor._name !== name) {
                    actor._name = name;
                    if (actor.icon?.label)
                        actor.icon.label.text = name;
                }
                actor.icon?.update?.();
            } else if (actor instanceof AppDisplay.FolderIcon) {
                actor.icon?.update?.();
            }
            for (const child of actor.get_children())
                walk(child);
        };
        try {
            walk(Main.uiGroup);
        } catch (e) {
            console.error(`Amethystora UI: appgrid: the icons drawn already: ${e.message}`);
        }
    }

    // --- The order ------------------------------------------------------------------------------------------------

    // GNOME places each icon where the account's saved layout says (_getItemPosition, by its page and place), and saves
    // the pages after every change. In an order, each top-level item gets its place from the order instead: the
    // folders first, as the account left them, then the apps. The saved layout is left alone meanwhile, so that GNOME's
    // own order comes back as it was.
    _wrapOrder() {
        const looks = this;
        const proto = AppDisplay.AppDisplay.prototype;
        this._injections.overrideMethod(proto, '_loadApps', original => function (...args) {
            const items = original.apply(this, args);
            try {
                looks._sort(this, items);
            } catch (e) {
                looks._sorted = null;
                console.error(`Amethystora UI: appgrid: the order, so GNOME's own is kept: ${e.message}`);
            }
            return items;
        });
        this._injections.overrideMethod(proto, '_getItemPosition', original => function (item) {
            const index = item && item !== this._placeholder ? looks._sorted?.get(item.id) : undefined;
            if (index === undefined)
                return original.call(this, item);
            const perPage = this._grid?.itemsPerPage ?? 0;
            return perPage > 0 ? [Math.floor(index / perPage), index % perPage] : [0, index];
        });
        this._injections.overrideMethod(proto, '_compareItems', original => function (a, b) {
            const sorted = looks._sorted;
            if (sorted?.has(a?.id) && sorted.has(b?.id))
                return sorted.get(a.id) - sorted.get(b.id);
            return original.call(this, a, b);
        });
        this._injections.overrideMethod(proto, '_savePages', original => function (...args) {
            if (looks._sorted)
                return undefined;
            return original.apply(this, args);
        });
    }

    _sort(display, items) {
        if (this._order === 'custom') {
            this._sorted = null;
            return;
        }
        const pages = display._pageManager;
        const saved = id => {
            const [page, position] = pages?.getAppPosition(id) ?? [-1, -1];
            return page < 0 ? Number.MAX_SAFE_INTEGER : page * 10000 + position;
        };
        const nameOf = item => item.app?.get_name() ?? item.name ?? '';
        const byName = (a, b) => nameOf(a).localeCompare(nameOf(b));
        const folders = items.filter(item => item instanceof AppDisplay.FolderIcon)
            .sort((a, b) => saved(a.id) - saved(b.id) || byName(a, b));
        const apps = items.filter(item => item instanceof AppDisplay.AppIcon && item !== display._placeholder);
        if (this._order === 'use') {
            const usage = Shell.AppUsage.get_default();
            apps.sort((a, b) => usage.compare(a.id, b.id) || byName(a, b));
        } else if (this._order === 'kind') {
            const kinds = new Map(apps.map(item => [item.id, kindOf(item.app)]));
            apps.sort((a, b) => kinds.get(a.id) - kinds.get(b.id) || byName(a, b));
        } else {
            apps.sort(byName);
        }
        this._sorted = new Map([...folders, ...apps].map((item, index) => [item.id, index]));
    }

    _redisplay() {
        try {
            this._appDisplay?._redisplay();
        } catch (e) {
            console.error(`Amethystora UI: appgrid: laying the grid out again: ${e.message}`);
        }
    }

    // --- The menu -------------------------------------------------------------------------------------------------

    _wrapMenu() {
        const looks = this;
        this._injections.overrideMethod(AppMenu.AppMenu.prototype, 'open', original => function (...args) {
            try {
                looks._prepare(this);
            } catch (e) {
                console.error(`Amethystora UI: appgrid: the app menu's items: ${e.message}`);
            }
            return original.apply(this, args);
        });
    }

    _prepare(menu) {
        if (!this._menus)
            return;
        const source = menu.sourceActor;
        const ours = source instanceof AppDisplay.AppIcon && !(source instanceof Dash.DashIcon);
        let items = menu._amethystoraLooks;
        if (!items) {
            if (!ours)
                return;
            const section = new PopupMenu.PopupMenuSection();
            const rename = new PopupMenu.PopupMenuItem(_('Rename…'));
            rename.connect('activate', () => this._rename(menu._app).catch(logError));
            const icon = new PopupMenu.PopupMenuItem(_('Change icon…'));
            icon.connect('activate', () => this._changeIcon(menu._app).catch(logError));
            const hide = new PopupMenu.PopupMenuItem(_('Hide from the grid and search'));
            hide.connect('activate', () => this._hide(menu._app));
            const reach = new PopupMenu.PopupMenuItem(_('What can it reach?'));
            reach.connect('activate', () => this._reach(menu._app));
            for (const item of [rename, icon, hide, reach])
                section.addMenuItem(item);
            // Right after Pin to Dash, where GNOME keeps what the app is to the account
            const index = menu._getMenuItems().indexOf(menu._toggleFavoriteItem);
            menu.addMenuItem(section, index < 0 ? undefined : index + 1);
            items = {section, rename, icon, hide, reach};
            menu._amethystoraLooks = items;
            menu.connectObject('destroy', () => this._menus?.delete(menu), this);
            this._menus.add(menu);
        }
        const app = menu._app;
        const id = app?.get_id() ?? '';
        const launcher = ours && LAUNCHER.test(id) && Boolean(app?.app_info);
        items.section.visible = launcher;
        items.reach.visible = launcher && flatpakId(app) !== null;
    }

    async _rename(app) {
        const id = app?.get_id();
        if (!this._settings || !LAUNCHER.test(id ?? ''))
            return;
        const own = app.app_info?.get_name() ?? id;
        const {dialog, answer} = askName(own, this._names.get(id) ?? '');
        this._track(dialog);
        const name = await answer;
        if (name === null || !this._settings)
            return;
        const chosen = clean(name);
        this._write('names', value => {
            if (chosen && chosen !== own)
                value[id] = chosen;
            else
                delete value[id];
        });
    }

    async _changeIcon(app) {
        const id = app?.get_id();
        if (!this._settings || !LAUNCHER.test(id ?? ''))
            return;
        const {dialog, answer} = askIcon(app.get_name(), this.ownIcon(app, 64), packsFor(app),
            Boolean(GLib.find_program_in_path('zenity')));
        this._track(dialog);
        const chosen = await answer;
        if (chosen === null || !this._settings)
            return;
        if (chosen === 'picture') {
            await this._picture(app);
            return;
        }
        this._write('icons', value => {
            if (PACK.test(chosen))
                value[id] = chosen;
            else
                delete value[id];
        });
    }

    // A picture of the account's own, picked in a file chooser outside the shell and made into a 256-pixel PNG by
    // amethystora-app-look as the account: the shell itself never decodes the picture that was picked
    async _picture(app) {
        Main.overview.hide();
        const chooser = Gio.Subprocess.new(['zenity', '--file-selection', `--title=${_('A picture for %s').format(app.get_name())}`,
            `--file-filter=${_('Pictures')} | *.png *.jpg *.jpeg *.svg *.webp`],
        Gio.SubprocessFlags.STDOUT_PIPE | Gio.SubprocessFlags.STDERR_SILENCE);
        const [path] = await chooser.communicate_utf8_async(null, null);
        const file = path?.trim();
        if (!chooser.get_successful() || !file?.startsWith('/') || !this._settings)
            return;
        const look = Gio.Subprocess.new([LOOK, 'icon', app.get_id(), 'picture', file],
            Gio.SubprocessFlags.STDOUT_SILENCE | Gio.SubprocessFlags.STDERR_SILENCE);
        await look.wait_async(null);
        if (!look.get_successful() && this._settings)
            this._notify(_('That picture could not be used'), _('Pick a PNG, JPEG, SVG or WebP picture.'));
    }

    _hide(app) {
        const id = app?.get_id();
        if (!this._settings || !LAUNCHER.test(id ?? ''))
            return;
        const name = app.get_name();
        this.hide(id, true);
        this._notify(_('%s is hidden').format(name),
            _('It still opens from a terminal and from Open With. Control’s Apps page shows it again.'),
            [_('Undo'), () => this._settings && this.hide(id, false)]);
    }

    // Security's Apps page on this app: what it can reach beyond its sandbox, and the controls Security has. Nothing
    // here switches anything.
    _reach(app) {
        const id = flatpakId(app);
        if (!id)
            return;
        try {
            Gio.Subprocess.new([SECURITY, `apps/${id}`], Gio.SubprocessFlags.STDOUT_SILENCE | Gio.SubprocessFlags.STDERR_SILENCE);
        } catch (e) {
            console.error(`Amethystora UI: appgrid: opening Security: ${e.message}`);
        }
        Main.overview.hide();
    }

    _track(dialog) {
        this._dialogs.add(dialog);
        dialog.connect('closed', () => this._dialogs?.delete(dialog));
    }

    _notify(title, body, action = null) {
        if (!this._source) {
            this._source = new MessageTray.Source({title: _('App grid'), iconName: 'view-app-grid-symbolic'});
            this._source.connect('destroy', () => (this._source = null));
            Main.messageTray.add(this._source);
        }
        const notification = new MessageTray.Notification({source: this._source, title, body, isTransient: true});
        if (action)
            notification.addAction(...action);
        this._source.addNotification(notification);
    }
}

// A name as the grid may show it: one line, no control or format character, 64 characters at most
export function clean(name) {
    const text = String(name ?? '').replace(/[\p{Cc}\p{Cf}]/gu, ' ').replace(/\s+/g, ' ').trim();
    return [...text].slice(0, NAME_MAX).join('');
}
