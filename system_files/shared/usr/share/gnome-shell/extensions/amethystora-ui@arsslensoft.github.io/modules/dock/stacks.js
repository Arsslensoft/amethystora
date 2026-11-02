// Folders kept in the dock (spec 1.10): Downloads by default, and any folder dragged there from Files. A stack shows
// the folder's own icon (a custom one too, so a folder's colour shows) and opens as a grid of the twelve newest
// things in it. The folder is read when the grid opens; nothing runs for a stack while it is shut, but for one thing:
// the Downloads stack counts what arrives in it ("+3" on its tile, R5-35) from a monitor of that one folder, and the
// count, kept in memory only, is gone once the stack or the folder is opened. Nothing is read or written for it.

import Clutter from 'gi://Clutter';
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import Pango from 'gi://Pango';
import St from 'gi://St';

import {gettext as _} from 'resource:///org/gnome/shell/extensions/extension.js';
import {EventEmitter} from 'resource:///org/gnome/shell/misc/signals.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';

import {PARTIAL, reportMoving} from '../pill/movinglist.js';
import {Popover} from './popover.js';
import {EASE, scaleFactor} from './util.js';

export const MOST_STACKS = 12;
const NEWEST = 12;
const MOST_READ = 5000;
const BATCH = 64;
const ICON = 44;
const GRID_ROW = 12;
const GRID_COLUMN = 8;
const GAP = 12;
const POP = {from: 16, scale: 0.94, open: 260, close: 160, openMode: EASE.quart, closeMode: EASE.quart};
// The stack that counts what arrives, and the names a browser gives a download until it is whole
const COUNTED = 'xdg:DOWNLOAD';
// Names remembered, so that a download made in two steps (an empty file, then the whole one renamed onto it) counts once
const MOST_NAMES = 200;
const ATTRIBUTES = 'standard::name,standard::display-name,standard::icon,standard::content-type,' +
    'standard::is-hidden,standard::type,time::modified,metadata::custom-icon-name';

// The icon Files shows for a file: a folder given a colour (Files' Scripts › Folder colour) carries the icon's name in
// metadata::custom-icon-name, looked up in the icon pack, with the file's own names behind it when they are names (the
// St.Icon's fallback_icon_name does the rest); a name that is not an icon name is ignored
function iconOf(info, fallback) {
    const own = info.get_icon() ?? fallback;
    const name = info.get_attribute_string('metadata::custom-icon-name');
    if (!name || !/^[A-Za-z0-9_.-]{1,64}$/.test(name))
        return own;
    return Gio.ThemedIcon.new_from_names([name, ...own instanceof Gio.ThemedIcon ? own.get_names() : []]);
}

const XDG = {
    DESKTOP: GLib.UserDirectory.DIRECTORY_DESKTOP,
    DOCUMENTS: GLib.UserDirectory.DIRECTORY_DOCUMENTS,
    DOWNLOAD: GLib.UserDirectory.DIRECTORY_DOWNLOAD,
    MUSIC: GLib.UserDirectory.DIRECTORY_MUSIC,
    PICTURES: GLib.UserDirectory.DIRECTORY_PICTURES,
    PUBLICSHARE: GLib.UserDirectory.DIRECTORY_PUBLIC_SHARE,
    TEMPLATES: GLib.UserDirectory.DIRECTORY_TEMPLATES,
    VIDEOS: GLib.UserDirectory.DIRECTORY_VIDEOS,
};

// The folder a value of the key names, or null with why
export function resolveStack(value) {
    if (typeof value !== 'string' || !value)
        return [null, 'empty'];
    if (value.startsWith('xdg:')) {
        const which = XDG[value.slice(4)];
        const path = which === undefined ? null : GLib.get_user_special_dir(which);
        return path ? [path, null] : [null, 'not a special folder of this account'];
    }
    if (value.startsWith('~/')) {
        const rest = value.slice(2);
        if (rest.split('/').includes('..'))
            return [null, '".." in the path'];
        return [GLib.build_filenamev([GLib.get_home_dir(), rest]), null];
    }
    if (value.startsWith('/') && !value.split('/').includes('..'))
        return [value, null];
    return [null, 'not a path'];
}

// The value the key keeps for a folder: a special folder by name, one under the home folder from it
export function stackValue(path) {
    for (const [name, which] of Object.entries(XDG)) {
        if (GLib.get_user_special_dir(which) === path)
            return `xdg:${name}`;
    }
    const home = GLib.get_home_dir();
    if (path.startsWith(`${home}/`))
        return `~/${path.slice(home.length + 1)}`;
    return path;
}

export class Stacks extends EventEmitter {
    constructor(dock) {
        super();
        this._dock = dock;
        this._places = [];
        this._said = new Set();
        this.reads = 0;
    }

    start() {
        this._cancellable = new Gio.Cancellable();
        this._dock.settings.connectObject('changed::stacks', () => this._read(), this);
        St.TextureCache.get_default().connectObject('icon-theme-changed', () => this._icons(), this);
        this._read();
    }

    stop() {
        this._cancellable?.cancel();
        this._cancellable = null;
        this._dock.settings.disconnectObject(this);
        St.TextureCache.get_default().disconnectObject(this);
        for (const place of this._places)
            this._unwatch(place);
        this._places = [];
    }

    // ---- What arrives in Downloads (R5-35) ----

    _watch(place) {
        if (place.value !== COUNTED || place.monitor)
            return;
        place.arrived = 0;
        place.names = new Set();
        try {
            place.monitor = Gio.File.new_for_path(place.path).monitor_directory(Gio.FileMonitorFlags.WATCH_MOVES, null);
        } catch (e) {
            this._say(`${place.value}: not watched: ${e.message}`);
            return;
        }
        place.monitor.connect('changed', (monitor, file, other, event) => this._changed(place, file, other, event));
    }

    _unwatch(place) {
        place.monitor?.cancel();
        place.monitor = null;
        place.arrived = 0;
        place.names = null;
    }

    // A whole file under a name of its own: made there, moved in, or a download's partial name turned into its own
    _changed(place, file, other, event) {
        const E = Gio.FileMonitorEvent;
        this._moving(file, event);
        let name = null;
        if (event === E.CREATED || event === E.MOVED_IN)
            name = file?.get_basename();
        else if (event === E.RENAMED && PARTIAL.test(file?.get_basename() ?? ''))
            name = other?.get_basename();
        if (!name || name.startsWith('.') || PARTIAL.test(name) || !place.names || place.names.has(name))
            return;
        if (place.names.size >= MOST_NAMES)
            place.names.clear();
        place.names.add(name);
        place.arrived++;
        this.emit('arrived', place);
    }

    // A download on its way, for the live pill's moving (pill/movinglist.js): only ever its name, there
    _moving(file, event) {
        const E = Gio.FileMonitorEvent;
        const name = file?.get_basename() ?? '';
        if (!PARTIAL.test(name))
            return;
        if (event === E.CREATED || event === E.CHANGED || event === E.MOVED_IN)
            reportMoving('download', name, {path: file.get_path()});
        else if (event === E.DELETED || event === E.RENAMED || event === E.MOVED_OUT)
            reportMoving('download', name, null);
    }

    // Opened: the count is gone
    _seen(place) {
        if (!place.arrived)
            return;
        place.arrived = 0;
        place.names?.clear();
        this.emit('arrived', place);
    }

    // For the suites: how many have arrived in a stack since it was last opened
    arrivals(value = COUNTED) {
        return this._places.find(place => place.value === value)?.arrived ?? 0;
    }

    list() {
        return [...this._places];
    }

    get values() {
        return this._places.map(place => place.value);
    }

    _read() {
        const values = this._dock.settings.get_strv('stacks').slice(0, MOST_STACKS);
        const places = [];
        for (const value of values) {
            const [path, why] = resolveStack(value);
            if (!path) {
                this._say(`${value}: ${why}`);
                continue;
            }
            if (places.some(place => place.path === path))
                continue;
            const old = this._places.find(place => place.path === path);
            const file = Gio.File.new_for_path(path);
            places.push(old ?? {
                stack: true,
                value,
                path,
                uri: file.get_uri(),
                name: GLib.path_get_basename(path),
                gicon: Gio.ThemedIcon.new('folder'),
                fallback: 'folder',
            });
            places[places.length - 1].value = value;
        }
        for (const place of this._places) {
            if (!places.includes(place))
                this._unwatch(place);
        }
        for (const place of places)
            this._watch(place);
        this._places = places;
        this._icons();
        this.emit('changed');
    }

    _say(message) {
        if (this._said.has(message))
            return;
        this._said.add(message);
        console.warn(`Amethystora UI: dock: a stack was left out: ${message}`);
    }

    // The folder's own icon and name, once per build, theme and opening
    _icons() {
        for (const place of this._places)
            this.refresh(place);
    }

    // The pointer came to the dock: a colour given to a stacked folder in Files since shows on its tile. One query per
    // stack, never a watch on a folder while its stack is shut
    reread() {
        this._icons();
    }

    refresh(place) {
        const file = Gio.File.new_for_path(place.path);
        file.query_info_async('standard::icon,standard::display-name,standard::type,metadata::custom-icon-name', Gio.FileQueryInfoFlags.NONE,
            GLib.PRIORITY_LOW, this._cancellable, (object, result) => {
                let info;
                try {
                    info = file.query_info_finish(result);
                } catch (e) {
                    if (!e.matches(Gio.IOErrorEnum, Gio.IOErrorEnum.CANCELLED))
                        this._say(`${place.value}: ${e.message}`);
                    return;
                }
                if (info.get_file_type() !== Gio.FileType.DIRECTORY) {
                    this._say(`${place.value}: not a folder`);
                    return;
                }
                const gicon = iconOf(info, place.gicon);
                const name = info.get_display_name() || place.name;
                // Read again as the pointer comes: the tile is drawn again only when something changed
                if (place.gicon?.equal(gicon) && place.name === name)
                    return;
                place.gicon = gicon;
                place.name = name;
                this.emit('place-changed', place);
            });
    }

    add(path) {
        const values = this._dock.settings.get_strv('stacks');
        const value = stackValue(path);
        if (values.includes(value) || values.length >= MOST_STACKS)
            return false;
        this._dock.settings.set_strv('stacks', [...values, value]);
        return true;
    }

    remove(place) {
        const values = this._dock.settings.get_strv('stacks').filter(value => resolveStack(value)[0] !== place.path);
        this._dock.settings.set_strv('stacks', values);
    }

    open(place) {
        this._seen(place);
        try {
            Gio.AppInfo.launch_default_for_uri(place.uri, global.create_app_launch_context(0, -1));
        } catch (e) {
            Main.notifyError(_('%s could not be opened').format(place.name), e.message);
        }
    }

    // ---- The grid ----

    // The twelve newest entries of the folder, folders included, hidden ones left out; at most MOST_READ looked at
    async newest(place, cancellable) {
        this.reads++;
        const file = Gio.File.new_for_path(place.path);
        const enumerator = await new Promise((resolve, reject) => file.enumerate_children_async(ATTRIBUTES,
            Gio.FileQueryInfoFlags.NOFOLLOW_SYMLINKS, GLib.PRIORITY_LOW, cancellable, (o, result) => {
                try {
                    resolve(file.enumerate_children_finish(result));
                } catch (e) {
                    reject(e);
                }
            }));
        const kept = [];
        let seen = 0;
        try {
            for (;;) {
                // eslint-disable-next-line no-await-in-loop
                const infos = await new Promise((resolve, reject) => enumerator.next_files_async(BATCH,
                    GLib.PRIORITY_LOW, cancellable, (o, result) => {
                        try {
                            resolve(enumerator.next_files_finish(result));
                        } catch (e) {
                            reject(e);
                        }
                    }));
                if (!infos.length)
                    break;
                for (const info of infos) {
                    seen++;
                    if (info.get_is_hidden() || info.get_name().startsWith('.'))
                        continue;
                    const time = info.get_modification_date_time()?.to_unix_usec() ?? 0;
                    if (kept.length === NEWEST && time <= kept[NEWEST - 1].time)
                        continue;
                    kept.push({info, time, file: enumerator.get_child(info)});
                    kept.sort((a, b) => b.time - a.time);
                    kept.length = Math.min(kept.length, NEWEST);
                }
                if (seen >= MOST_READ)
                    break;
            }
        } finally {
            enumerator.close_async(GLib.PRIORITY_LOW, null, null);
        }
        return kept;
    }

    // Opens the grid over a stack's item
    show(dash, state) {
        const place = state.key;
        const cancellable = new Gio.Cancellable();
        this._seen(place);
        this.refresh(place);
        const popover = new Popover(this._dock, dash, state, {
            styleClass: 'amethystora-dock-stack',
            gap: GAP,
            pop: POP,
            onClose: () => {
                cancellable.cancel();
                this.refresh(place);
                this._dock.cardClosed(popover);
            },
        });
        const card = popover.card;
        const header = new St.BoxLayout({style_class: 'amethystora-dock-stack-header', x_expand: true});
        const title = new St.Label({
            style_class: 'amethystora-dock-stack-name', text: place.name, x_expand: true,
            y_align: Clutter.ActorAlign.CENTER,
        });
        title.clutter_text.use_markup = false;
        title.clutter_text.ellipsize = Pango.EllipsizeMode.END;
        header.add_child(title);
        header.add_child(new St.Label({
            style_class: 'amethystora-dock-stack-chip', text: _('Newest first'),
            y_align: Clutter.ActorAlign.CENTER,
        }));
        card.add_child(header);
        const body = new St.Widget({
            style_class: 'amethystora-dock-stack-grid',
            layout_manager: new Clutter.GridLayout({
                column_homogeneous: true,
                row_spacing: GRID_ROW * scaleFactor(),
                column_spacing: GRID_COLUMN * scaleFactor(),
            }),
            x_expand: true,
        });
        card.add_child(body);
        const footer = new St.Button({
            style_class: 'amethystora-dock-stack-open',
            label: _('Open in Files'),
            can_focus: true,
            x_align: Clutter.ActorAlign.CENTER,
        });
        footer.connect('clicked', () => {
            popover.close();
            this.open(place);
        });
        card.add_child(footer);
        popover.open();
        this.entries = [];

        this.newest(place, cancellable).then(kept => {
            if (cancellable.is_cancelled())
                return;
            this._fill(popover, body, kept);
        }).catch(e => {
            if (!e.matches?.(Gio.IOErrorEnum, Gio.IOErrorEnum.CANCELLED))
                this._fill(popover, body, [], e.message);
        });
        return popover;
    }

    _fill(popover, body, kept, problem = null) {
        const layout = body.layout_manager;
        this.entries = kept.map(entry => entry.info.get_display_name());
        if (!kept.length) {
            const empty = new St.Label({
                style_class: 'amethystora-dock-stack-empty',
                text: problem ? _('This folder could not be read') : _('Nothing here yet'),
                x_align: Clutter.ActorAlign.CENTER, x_expand: true,
            });
            layout.attach(empty, 0, 0, 3, 1);
        }
        kept.forEach((entry, i) => {
            const button = new St.Button({
                style_class: 'amethystora-dock-stack-entry', can_focus: true,
                x_expand: true,
            });
            const box = new St.BoxLayout({orientation: Clutter.Orientation.VERTICAL, x_expand: true});
            const gicon = iconOf(entry.info, Gio.ThemedIcon.new('text-x-generic'));
            box.add_child(new St.Icon({
                gicon, icon_size: ICON, x_align: Clutter.ActorAlign.CENTER,
                fallback_icon_name: 'text-x-generic',
            }));
            const name = new St.Label({
                style_class: 'amethystora-dock-stack-entry-name',
                text: entry.info.get_display_name(), x_align: Clutter.ActorAlign.CENTER,
            });
            name.clutter_text.use_markup = false;
            name.clutter_text.ellipsize = Pango.EllipsizeMode.END;
            box.add_child(name);
            button.child = box;
            button.accessible_name = entry.info.get_display_name();
            button.connect('clicked', () => {
                popover.close();
                try {
                    Gio.AppInfo.launch_default_for_uri(entry.file.get_uri(),
                        global.create_app_launch_context(0, -1));
                } catch (e) {
                    Main.notifyError(_('%s could not be opened').format(entry.info.get_display_name()), e.message);
                }
            });
            layout.attach(button, i % 3, Math.floor(i / 3), 1, 1);
            if (i === 0)
                popover.focusFirst(button);
        });
        popover.place();
    }
}
