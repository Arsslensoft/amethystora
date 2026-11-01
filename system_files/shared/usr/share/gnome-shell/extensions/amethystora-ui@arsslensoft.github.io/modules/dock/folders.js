// Folders of pinned apps (spec 1.9). Hold one pinned app over the middle of another until it lights up and let go: the
// two become a folder, named Folder (Folder 2, Folder 3...). An app dropped the same way on a folder joins it. A folder
// stands where its first app stood; a click opens it as a card of its apps with its name under them, and a click on
// the name renames it. Its menu takes it apart again. The apps stay pinned, in GNOME's order, so that Alt+1..9, the
// app grid's Unpin and the setup file go on as before: a folder only groups them on the dock.

import Clutter from 'gi://Clutter';
import GLib from 'gi://GLib';
import Pango from 'gi://Pango';
import Shell from 'gi://Shell';
import St from 'gi://St';

import {gettext as _} from 'resource:///org/gnome/shell/extensions/extension.js';
import {EventEmitter} from 'resource:///org/gnome/shell/misc/signals.js';
import * as AppFavorites from 'resource:///org/gnome/shell/ui/appFavorites.js';
import * as DND from 'resource:///org/gnome/shell/ui/dnd.js';
import * as Dash from 'resource:///org/gnome/shell/ui/dash.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import * as PopupMenu from 'resource:///org/gnome/shell/ui/popupMenu.js';

import {Popover} from './popover.js';
import {EASE, scaleFactor} from './util.js';

const HOLD = 300;
const BAND = 0.4;
const GROWN = 1.18;
const GROW = 200;
const LONGEST_NAME = 40;
const CARD_GAP = 14;
const POP = {from: 16, scale: 0.94, open: 260, close: 160, openMode: EASE.quart, closeMode: EASE.quart};
const MEMBER_ICON = 48;
const GRID = {padding: 6, icon: 16, gap: 4};
const APP_ID = /^[A-Za-z0-9._+-]+\.desktop$/;
const UNSEEN = /[\p{Cc}\p{Cf}]/u;

export function validName(name, others = []) {
    return typeof name === 'string' && name.trim() !== '' && [...name].length <= LONGEST_NAME && !UNSEEN.test(name) &&
        !others.includes(name);
}

// The folder's picture: its first four apps in a 2 x 2 grid on a rounded tile. Built at `drawn` px and shown at `size`
// px, so that it is sharp where the dock grows it to `drawn`.
export function folderPicture(apps, size, drawn) {
    const sf = scaleFactor();
    const f = drawn / 48;
    const holder = new St.Widget({width: size * sf, height: size * sf, layout_manager: new Clutter.FixedLayout()});
    const tile = new St.Widget({
        style_class: 'amethystora-dock-folder-icon',
        width: drawn * sf,
        height: drawn * sf,
        layout_manager: new Clutter.FixedLayout(),
    });
    tile.set_style(`border-radius: ${Math.round(12 * f)}px;`);
    const icon = Math.round(GRID.icon * f);
    apps.slice(0, 4).forEach((app, i) => {
        const texture = app.create_icon_texture(icon);
        texture.set_position(Math.round((GRID.padding + (i % 2) * (GRID.icon + GRID.gap)) * f * sf),
            Math.round((GRID.padding + Math.floor(i / 2) * (GRID.icon + GRID.gap)) * f * sf));
        tile.add_child(texture);
    });
    tile.set_pivot_point(0, 0);
    tile.set_scale(size / drawn, size / drawn);
    holder.add_child(tile);
    return holder;
}

export class Folders extends EventEmitter {
    constructor(dock) {
        super();
        this._dock = dock;
        this._list = [];
        this._stored = [];
        this._said = new Set();
    }

    start() {
        this._dock.settings.connectObject('changed::folders', () => this._read(), this);
        // Shown again whenever GNOME's pinned apps change, which an app installed, updated or hidden by Parental
        // Controls does too; written only when the pinned apps themselves, the key, lost one
        AppFavorites.getAppFavorites().connectObject('changed', () => this._read(), this);
        global.settings.connectObject('changed::favorite-apps', () => this._read(true), this);
        Main.overview.connectObject(
            'item-drag-begin', () => this._dragBegan(),
            'item-drag-end', () => this._dragEnded(),
            'item-drag-cancelled', () => this._dragEnded(), this);
        this._read();
    }

    stop() {
        this._dock.settings.disconnectObject(this);
        AppFavorites.getAppFavorites().disconnectObject(this);
        global.settings.disconnectObject(this);
        Main.overview.disconnectObject(this);
        this._dragEnded();
        this._list = [];
    }

    // [{name, ids}] that can be used: two pinned apps or more, in the key's order
    get list() {
        return this._list;
    }

    get value() {
        return Object.fromEntries(this._stored.map(folder => [folder.name, [...folder.ids]]));
    }

    folderOf(id) {
        return this._list.find(folder => folder.ids.includes(id)) ?? null;
    }

    // unpinned: GNOME's list of pinned apps (the key) changed, so an app unpinned there leaves its folder for good,
    // and a folder left with one app is taken apart. An app that is only not installed for a moment, or hidden by
    // Parental Controls, keeps its place: it is left out of what is shown, never out of the key.
    _read(unpinned = false) {
        const raw = this._dock.settings.get_value('folders').deepUnpack();
        const pinned = global.settings.get_strv('favorite-apps');
        const favorites = AppFavorites.getAppFavorites();
        const system = Shell.AppSystem.get_default();
        const used = new Set();
        const list = [];
        const stored = [];
        let unpinnedOne = false;
        for (const [name, ids] of Object.entries(raw)) {
            if (!validName(name)) {
                this._say(`the folder name ${JSON.stringify(name)} is not one a folder can have`);
                continue;
            }
            const members = ids.filter(id => APP_ID.test(id) && pinned.includes(id) && !used.has(id));
            if (members.length !== ids.length)
                unpinnedOne = true;
            members.forEach(id => used.add(id));
            if (members.length >= 2)
                stored.push({name, ids: members});
            const shown = members.filter(id => system.lookup_app(id) && favorites.isFavorite(id));
            if (shown.length < 2) {
                this._say(`the folder ${name} has fewer than two pinned apps`);
                continue;
            }
            list.push({name, ids: shown});
        }
        this._list = list;
        this._stored = stored;
        if (unpinned && unpinnedOne)
            this._write(stored);
        this.emit('changed');
    }

    _say(message) {
        if (this._said.has(message))
            return;
        this._said.add(message);
        console.warn(`Amethystora UI: dock: ${message}`);
    }

    _write(list) {
        const value = Object.fromEntries(list.map(folder => [folder.name, folder.ids]));
        this._dock.settings.set_value('folders', new GLib.Variant('a{sas}', value));
    }

    _nextName() {
        const names = this._stored.map(folder => folder.name);
        const base = _('Folder');
        if (!names.includes(base))
            return base;
        for (let n = 2; ; n++) {
            // Translators: a folder's name when a folder of that name is there already
            const name = _('Folder %d').format(n);
            if (!names.includes(name))
                return name;
        }
    }

    make(targetId, droppedId) {
        const list = this._stored.filter(folder => !folder.ids.includes(droppedId));
        const into = list.find(folder => folder.ids.includes(targetId));
        if (into)
            into.ids = [...into.ids, droppedId];
        else
            list.push({name: this._nextName(), ids: [targetId, droppedId]});
        this._write(list);
    }

    rename(folder, name) {
        const others = this._stored.filter(other => other.name !== folder.name).map(other => other.name);
        if (!validName(name, others))
            return false;
        this._write(this._stored.map(other => other.name === folder.name ? {name, ids: other.ids} : other));
        return true;
    }

    takeApart(folder) {
        this._write(this._stored.filter(other => other.name !== folder.name));
    }

    leave(folder, id) {
        this._write(this._stored.map(other => other.name === folder.name
            ? {name: other.name, ids: other.ids.filter(member => member !== id)} : other));
    }

    // ---- Making one by dropping ----

    _dragBegan() {
        this._monitor = {
            dragMotion: event => {
                if (this._hover && !this._hover.actor.contains(event.targetActor))
                    this._unhover();
                return DND.DragMotionResult.CONTINUE;
            },
        };
        DND.addDragMonitor(this._monitor);
    }

    _dragEnded() {
        if (this._monitor)
            DND.removeDragMonitor(this._monitor);
        this._monitor = null;
        this._unhover();
    }

    // A shell drag over a pinned app's icon or a folder: target {actor, visual, id, folder}, x and y in the actor's
    // own coordinates. MOVE_DROP while the middle of it is held; CONTINUE elsewhere, which leaves it to GNOME's dash.
    over(dash, target, source, x, y) {
        const app = Dash.Dash.getAppFromSource(source);
        const favorites = AppFavorites.getAppFavorites();
        if (!app || !favorites.isFavorite(app.get_id()))
            return DND.DragMotionResult.CONTINUE;
        const id = app.get_id();
        if (id === target.id || target.folder?.ids.includes(id)) {
            this._unhover();
            return DND.DragMotionResult.CONTINUE;
        }
        const {long} = dash.edge;
        const length = long === 'x' ? target.actor.width : target.actor.height;
        const at = long === 'x' ? x : y;
        const band = at >= length * (1 - BAND) / 2 && at <= length * (1 + BAND) / 2;
        if (!band) {
            if (this._hover?.actor === target.actor)
                this._unhover();
            return DND.DragMotionResult.CONTINUE;
        }
        if (this._hover?.actor !== target.actor) {
            this._unhover();
            this._hover = {...target, dash, lit: false};
            const hover = this._hover;
            hover.timer = GLib.timeout_add(GLib.PRIORITY_DEFAULT, HOLD, () => {
                hover.timer = 0;
                this._light(hover);
                return GLib.SOURCE_REMOVE;
            });
        }
        // GNOME's gap stays where the drag left it until the item lights up: let go before, the drop is GNOME's, which
        // moves the app there (accept() declines it); lit, the gap closes and the drop makes the folder
        if (this._hover.lit)
            dash._clearDragPlaceholder();
        return DND.DragMotionResult.MOVE_DROP;
    }

    accept(target, source) {
        const app = Dash.Dash.getAppFromSource(source);
        if (!app || this._hover?.actor !== target.actor || !this._hover.lit)
            return false;
        this.make(target.folder ? target.folder.ids[0] : target.id, app.get_id());
        this._unhover();
        return true;
    }

    _light(hover) {
        hover.lit = true;
        hover.actor.add_style_class_name('amethystora-dock-folder-target');
        const visual = hover.visual;
        if (!visual)
            return;
        visual._amethystoraBusy = true;
        visual.ease({scale_x: GROWN, scale_y: GROWN, duration: GROW, mode: EASE.out});
    }

    _unhover() {
        const hover = this._hover;
        this._hover = null;
        if (!hover)
            return;
        if (hover.timer)
            GLib.source_remove(hover.timer);
        hover.actor.remove_style_class_name('amethystora-dock-folder-target');
        if (hover.lit && hover.visual) {
            hover.visual.ease({
                scale_x: 1, scale_y: 1, duration: GROW, mode: EASE.out,
                onComplete: () => (hover.visual._amethystoraBusy = false),
            });
        }
    }

    // ---- The card ----

    show(dash, state, folder) {
        const system = Shell.AppSystem.get_default();
        const popover = new Popover(this._dock, dash, state, {
            styleClass: 'amethystora-dock-folder',
            gap: CARD_GAP,
            pop: POP,
            onClose: () => this._dock.cardClosed(popover),
        });
        const card = popover.card;
        const row = new St.BoxLayout({style_class: 'amethystora-dock-folder-row', x_align: Clutter.ActorAlign.CENTER});
        let first = null;
        for (const id of folder.ids) {
            const app = system.lookup_app(id);
            if (!app)
                continue;
            const button = new St.Button({style_class: 'amethystora-dock-folder-member', can_focus: true});
            const box = new St.BoxLayout({orientation: Clutter.Orientation.VERTICAL});
            const icon = app.create_icon_texture(MEMBER_ICON);
            icon.x_align = Clutter.ActorAlign.CENTER;
            box.add_child(icon);
            const name = new St.Label({
                style_class: 'amethystora-dock-folder-member-name', text: app.get_name(),
                x_align: Clutter.ActorAlign.CENTER,
            });
            name.clutter_text.use_markup = false;
            name.clutter_text.ellipsize = Pango.EllipsizeMode.END;
            box.add_child(name);
            button.child = box;
            button.accessible_name = app.get_name();
            button.connect('clicked', () => {
                popover.close();
                dash.clickApp(app);
            });
            this._dock.dissolve?.memberDraggable(button, app, folder, popover);
            row.add_child(button);
            first ??= button;
        }
        card.add_child(row);
        card.add_child(this._nameRow(folder, popover));
        popover.open();
        popover.focusFirst(first);
        return popover;
    }

    _nameRow(folder, popover) {
        const holder = new St.Bin({x_align: Clutter.ActorAlign.CENTER});
        const showName = () => {
            const button = new St.Button({style_class: 'amethystora-dock-folder-name', can_focus: true});
            const label = new St.Label({text: folder.name});
            label.clutter_text.use_markup = false;
            button.child = label;
            button.accessible_name = _('Rename %s').format(folder.name);
            button.connect('clicked', () => edit());
            holder.child = button;
        };
        const edit = () => {
            const entry = new St.Entry({
                style_class: 'amethystora-dock-folder-entry', text: folder.name,
                can_focus: true,
            });
            entry.clutter_text.use_markup = false;
            entry.clutter_text.max_length = LONGEST_NAME;
            entry.clutter_text.connect('activate', () => {
                if (this.rename(folder, entry.text.trim()))
                    folder = {...folder, name: entry.text.trim()};
                showName();
            });
            entry.clutter_text.connect('key-press-event', (actor, event) => {
                if (event.get_key_symbol() === Clutter.KEY_Escape) {
                    showName();
                    holder.child.grab_key_focus();
                    return Clutter.EVENT_STOP;
                }
                return Clutter.EVENT_PROPAGATE;
            });
            holder.child = entry;
            entry.grab_key_focus();
            entry.clutter_text.set_selection(0, -1);
            popover.place();
        };
        showName();
        return holder;
    }

    // ---- Its menu ----

    fillMenu(menu, folder, dash, state) {
        menu.removeAll();
        const heading = new PopupMenu.PopupMenuItem(folder.name, {reactive: false, can_focus: false});
        heading.label.clutter_text.use_markup = false;
        heading.add_style_class_name('amethystora-dock-menu-heading');
        menu.addMenuItem(heading);
        menu.addAction(_('Rename'), () => {
            const popover = this._dock.openCard(dash, state);
            if (popover?.card)
                this._renameIn(popover);
        });
        menu.addAction(_('Take the folder apart'), () => this.takeApart(folder));
    }

    _renameIn(popover) {
        const name = popover.card.get_last_child()?.child;
        if (name instanceof St.Button)
            name.emit('clicked', 1);
    }
}

