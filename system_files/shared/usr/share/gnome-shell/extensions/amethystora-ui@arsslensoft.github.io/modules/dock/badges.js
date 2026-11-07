// What an app's icon says about the app, on the dock and in the overview alike (the app grid, app folders and the
// search's results): the count of its unread notifications, or the count it gives itself, in a corner; a bar along
// the bottom for a task under way; and, while the app is being updated, the icon at half strength and a word instead
// of a launch when it is clicked. The counts come from the message tray, the rest from the launcher protocol
// (dock/launcher.js), which says nothing while notifications are silenced. The dock adds its own on top (dash.js).
// A terminal's icon counts its windows instead: its notifications are its commands' (one ended, one wants attention),
// read in the window and left to pile up in the tray, so a count of them said nothing about the terminal.

import Clutter from 'gi://Clutter';
import Gio from 'gi://Gio';
import GObject from 'gi://GObject';
import St from 'gi://St';

import {gettext as _, ngettext} from 'resource:///org/gnome/shell/extensions/extension.js';
import {EventEmitter} from 'resource:///org/gnome/shell/misc/signals.js';
import * as AppDisplay from 'resource:///org/gnome/shell/ui/appDisplay.js';
import * as Dash from 'resource:///org/gnome/shell/ui/dash.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';

const PROGRESS_FADE = 300;
// How large the count is beside a smaller icon: [icon size from, share of its size at 48 px]
const SHRINK = [[48, 1], [32, 0.87], [24, 0.65], [22, 0.6]];

// A count short enough for a corner: thousands, millions and billions as the language writes them, with a decimal
// while it says something
function shortCount(count) {
    if (count <= 9999)
        return count.toLocaleString(undefined, {useGrouping: false});
    const fractions = Math.floor(Math.log10(count)) % 3 === 2 ? 0 : 1;
    return new Intl.NumberFormat(undefined, {notation: 'compact', maximumFractionDigits: fractions}).format(count);
}

// A terminal emulator, by its launcher's categories
function isTerminal(app) {
    return (app.get_app_info()?.get_categories() ?? '').split(';').includes('TerminalEmulator');
}

// A terminal's windows, from two: one is already the running app's mark
function terminalWindows(app) {
    const count = app.get_windows().filter(window => !window.skip_taskbar).length;
    return count > 1 ? count : 0;
}

// Said in place of opening an app that is being updated
export function sayUpdating(name) {
    Main.osdWindowManager.showAll(Gio.ThemedIcon.new('action-unavailable-symbolic'),
        _('%s is being updated. Try again later.').format(name));
}

// The count and the bar of one icon, in parent, which is as large as the icon's tile; inset is the room in the tile
// around the picture of the icon. The picture itself is the BaseIcon's.
export class IconBadge {
    constructor(parent, baseIcon, inset) {
        this._baseIcon = baseIcon;
        this._inset = inset;
        this.count = 0;
        this.badge = new St.Label({
            style_class: 'amethystora-dock-badge',
            x_align: Clutter.ActorAlign.END,
            y_align: Clutter.ActorAlign.START,
            x_expand: true,
            y_expand: true,
            visible: false,
        });
        this.progress = new St.Widget({
            style_class: 'amethystora-dock-progress',
            layout_manager: new Clutter.BinLayout(),
            y_align: Clutter.ActorAlign.END,
            x_expand: true,
            y_expand: true,
            visible: false,
        });
        // From the start side, which Clutter turns round for right-to-left languages
        this._fill = new St.Widget({
            style_class: 'amethystora-dock-progress-fill',
            x_align: Clutter.ActorAlign.START,
            y_expand: true,
        });
        this._value = 0;
        this.progress.add_child(this._fill);
        this.progress.connect('notify::width', () => this._syncFill());
        parent.add_child(this.progress);
        parent.add_child(this.badge);
    }

    destroy() {
        this.badge.destroy();
        this.progress.destroy();
        this.dim(false);
    }

    // count: the unread items; entry: what the app said through the launcher protocol, or null. Says whether the
    // count changed.
    sync(count, entry) {
        this.badge.visible = count > 0;
        this.badge.text = count > 0 ? shortCount(count) : '';
        const progress = entry?.progressVisible ? Math.clamp(Number(entry.progress) || 0, 0, 1) : null;
        this._showProgress(progress !== null);
        this._value = progress ?? this._value;
        this._syncFill();
        this.dim(Boolean(entry?.updating || entry?.greyed));
        const changed = count !== this.count;
        this.count = count;
        return changed;
    }

    // The bar fades in and out over 300 ms
    _showProgress(on) {
        if (on === this._progressOn)
            return;
        this._progressOn = on;
        const bar = this.progress;
        bar.remove_all_transitions();
        if (on) {
            bar.visible = true;
            bar.opacity = 0;
        }
        bar.ease({
            opacity: on ? 255 : 0,
            duration: PROGRESS_FADE,
            mode: Clutter.AnimationMode.EASE_OUT_QUAD,
            onComplete: () => {
                if (!this._progressOn)
                    bar.visible = false;
            },
        });
    }

    // Half strength while the app is being updated. On the icon's holder, which keeps it when the icon is drawn
    // again at a new size.
    dim(on) {
        const holder = this._baseIcon?._iconBin;
        if (holder)
            holder.opacity = on ? 128 : 255;
    }

    // The bar and the count scale with the icon
    resize(size) {
        // A thin bar inside the icon: 4 px in from its sides, 2 px above its bottom, 5 px tall at a 48 px icon
        const f = size / 48;
        const side = this._inset + Math.round(4 * f);
        const bottom = this._inset + Math.round(2 * f);
        const height = Math.max(3, Math.round(5 * f));
        this.progress.set_style(`margin: 0 ${side}px ${bottom}px ${side}px; height: ${height}px;`);
        // The count shrinks less than the icon does, and never below half
        const shrink = SHRINK.find(([smallest]) => size >= smallest)?.[1] ?? 0.5;
        this.badge.set_style(`font-size: ${(0.9 * shrink).toFixed(2)}em; margin-left: ${Math.round(3 * shrink)}px;`);
    }

    _syncFill() {
        if (!this.progress.visible || !this.progress.has_allocation())
            return;
        const box = this.progress.get_theme_node().get_content_box(this.progress.get_allocation_box());
        this._fill.width = Math.round((box.x2 - box.x1) * this._value);
    }
}

// In the overview an icon's tile also holds its name: the count and the bar go over the picture alone, wherever the
// tile puts it. Its place is worked out from the allocations above it, which its tile has just made.
const Overlay = GObject.registerClass(
class AmethystoraDockOverlay extends St.Widget {
    _init(target) {
        super._init({layout_manager: new Clutter.BinLayout()});
        this._target = target;
    }

    vfunc_get_preferred_width() {
        return [0, 0];
    }

    vfunc_get_preferred_height() {
        return [0, 0];
    }

    vfunc_allocate(_box) {
        const parent = this.get_parent();
        const own = this._target.get_allocation_box();
        let [x, y] = [own.x1, own.y1];
        for (let actor = this._target.get_parent(); actor && actor !== parent; actor = actor.get_parent()) {
            const around = actor.get_allocation_box();
            x += around.x1;
            y += around.y1;
        }
        super.vfunc_allocate(new Clutter.ActorBox({
            x1: x,
            y1: y,
            x2: x + own.get_width(),
            y2: y + own.get_height(),
        }));
    }
});

export class Badges extends EventEmitter {
    constructor(launcher) {
        super();
        this._launcher = launcher;
    }

    start() {
        this._counts = null;
        this._notifications = new Set();
        // The overview's icons by app, as the launcher protocol names them
        this._icons = new Map();
        Main.messageTray.connectObject(
            'source-added', (tray, source) => this._watchSource(source),
            'source-removed', (tray, source) => {
                source.disconnectObject(this);
                this._trayChanged();
            }, this);
        for (const source of Main.messageTray.getSources())
            this._watchSource(source);
        this._launcher.connectObject('changed', (o, id) => {
            this._syncIcons(id);
            this.emit('changed', id);
        }, this);
        this._wrapIcons();
    }

    stop() {
        Main.messageTray.disconnectObject(this);
        for (const source of Main.messageTray.getSources())
            source.disconnectObject(this);
        for (const notification of this._notifications ?? [])
            notification.disconnectObject(this);
        this._notifications = null;
        this._launcher.disconnectObject(this);
        for (const [proto, name, original] of this._wrapped ?? [])
            proto[name] = original;
        this._wrapped = null;
        for (const icons of this._icons?.values() ?? []) {
            for (const icon of icons)
                this._undecorate(icon);
        }
        this._icons = null;
        this._counts = null;
    }

    // Notifications in the tray, but resident ones already seen; an app's own count above zero instead. None while
    // notifications are silenced.
    unread(app) {
        const entry = this._launcher.get(app.get_id());
        if (entry?.countVisible && Number(entry.count) > 0)
            return Number(entry.count);
        if (isTerminal(app))
            return terminalWindows(app);
        if (this._launcher.quiet)
            return 0;
        if (!this._counts) {
            this._counts = new Map();
            for (const source of Main.messageTray.getSources()) {
                const owner = source.app ?? source._app;
                if (!owner)
                    continue;
                const shown = source.notifications.filter(n => !(n.resident && n.acknowledged)).length;
                this._counts.set(owner, (this._counts.get(owner) ?? 0) + shown);
            }
        }
        return this._counts.get(app) ?? 0;
    }

    // The name an icon is read out by, with its count
    static accessibleName(name, count, app = null) {
        if (count > 0 && app && isTerminal(app))
            return ngettext('%s, %d window', '%s, %d windows', count).format(name, count);
        return count > 0 ? ngettext('%s, %d unread', '%s, %d unread', count).format(name, count) : name;
    }

    _watchSource(source) {
        source.connectObject(
            'notify::count', () => this._trayChanged(),
            'notification-added', (o, notification) => this._watchNotification(notification),
            this);
        source.notifications.forEach(notification => this._watchNotification(notification));
        this._trayChanged();
    }

    // A resident notification that has been seen stops counting at once
    _watchNotification(notification) {
        if (!this._notifications || this._notifications.has(notification))
            return;
        this._notifications.add(notification);
        notification.connectObject(
            'notify::acknowledged', () => this._trayChanged(),
            'destroy', () => {
                notification.disconnectObject(this);
                this._notifications?.delete(notification);
            }, this);
    }

    _trayChanged() {
        this._counts = null;
        this._syncIcons(null);
        this.emit('changed', null);
    }

    // ---- The overview's icons ----

    // Every app icon GNOME makes from now on gets its badge, and the ones it has made already; the dock's own icons
    // have theirs from the dock
    _wrapIcons() {
        const badges = this;
        const proto = AppDisplay.AppIcon.prototype;
        this._wrapped = [[proto, '_init', proto._init], [proto, 'activate', proto.activate]];
        const [[, , init], [, , activate]] = this._wrapped;
        proto._init = function (...args) {
            init.apply(this, args);
            if (!(this instanceof Dash.DashIcon))
                badges._decorate(this);
        };
        proto.activate = function (...args) {
            if (!(this instanceof Dash.DashIcon) && badges._launcher.get(this.app.get_id())?.updating) {
                sayUpdating(this.app.get_name());
                return;
            }
            activate.apply(this, args);
        };

        const controls = Main.overview._overview?._controls;
        const views = [controls?._appDisplay];
        for (const item of views[0]?._orderedItems ?? []) {
            if (item instanceof AppDisplay.FolderIcon)
                views.push(item.view);
        }
        const icons = views.flatMap(view => view?._orderedItems ?? []);
        for (const provider of controls?._searchController?._searchResults?._providers ?? [])
            icons.push(...Object.values(provider.display?._resultDisplays ?? {}));
        for (const icon of icons) {
            if (icon instanceof AppDisplay.AppIcon && !(icon instanceof Dash.DashIcon))
                this._decorate(icon);
        }
    }

    _decorate(icon) {
        const id = icon.app?.get_id();
        if (!id || !this._icons)
            return;
        if (!this._icons.has(id))
            this._icons.set(id, new Set());
        this._icons.get(id).add(icon);
        icon.connectObject('destroy', () => {
            this._icons?.get(id)?.delete(icon);
            icon._amethystoraBadge = null;
        }, this);
        if (isTerminal(icon.app))
            icon.app.connectObject('windows-changed', () => this._syncIcon(icon), icon);
        this._syncIcon(icon);
    }

    _undecorate(icon) {
        icon.disconnectObject(this);
        icon.app?.disconnectObject(icon);
        icon._amethystoraBadge?.destroy();
        icon._amethystoraOverlay?.destroy();
        icon._amethystoraBadge = icon._amethystoraOverlay = null;
        if ('_amethystoraName' in icon) {
            icon.accessible_name = icon._amethystoraName;
            delete icon._amethystoraName;
        }
    }

    _syncIcons(id) {
        if (!this._icons)
            return;
        const sets = id ? [this._icons.get(id) ?? []] : this._icons.values();
        for (const icons of sets) {
            for (const icon of icons)
                this._syncIcon(icon);
        }
    }

    // The overlay is only made once an icon has something to show
    _syncIcon(icon) {
        const entry = this._launcher.get(icon.app.get_id());
        const count = this.unread(icon.app);
        let badge = icon._amethystoraBadge;
        if (!badge) {
            if (!count && !entry?.progressVisible && !entry?.updating && !entry?.greyed)
                return;
            const overlay = new Overlay(icon.icon._iconBin);
            icon._iconContainer.add_child(overlay);
            badge = new IconBadge(overlay, icon.icon, 0);
            icon._amethystoraOverlay = overlay;
            icon._amethystoraBadge = badge;
            badge.resize(icon.icon.iconSize);
            icon.icon._iconBin.connectObject('notify::child', () => badge.resize(icon.icon.iconSize), overlay);
        }
        if (badge.sync(count, entry)) {
            if (!('_amethystoraName' in icon))
                icon._amethystoraName = icon.accessible_name;
            icon.accessible_name = count > 0
                ? Badges.accessibleName(icon.app.get_name(), count, icon.app) : icon._amethystoraName;
        }
    }
}
