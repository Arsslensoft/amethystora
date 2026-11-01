// Dragged off the dock and let go where nothing takes it (spec 1.13): a pinned app, a folder or a stack is taken out
// of the dock and dissolves where it was let go, its icon shrinking away while ten specks of glass burst out of it.
// Let go within 24 px of the pill, or the drag ended with Escape, and it goes back as before. Nothing is uninstalled:
// an app stays in the app grid, and a stack's folder is left as it is.

import Clutter from 'gi://Clutter';
import GLib from 'gi://GLib';
import Meta from 'gi://Meta';
import St from 'gi://St';

import * as AppFavorites from 'resource:///org/gnome/shell/ui/appFavorites.js';
import * as DND from 'resource:///org/gnome/shell/ui/dnd.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';

import {EASE, REDUCED_FADE, inside, scaleFactor} from './util.js';

const NEAR = 24;
const SHRINK = {duration: 500, scale: 0.6};
// The specks: diameter and where each flies to, y away from the edge as negative
const SPECKS = [
    [6, -34, -70], [9, -18, -96], [12, 0, -110], [6, 20, -92], [9, 36, -66],
    [12, -44, -40], [6, 44, -38], [9, -8, -60], [12, 12, -128], [6, -26, -122],
];
const SPECK = {stagger: 18, burst: 94, burstShare: 0.3, fly: 530, end: 0.5};

export class Dissolve {
    constructor(dock) {
        this._dock = dock;
        this.count = 0;
        this.playing = null;
    }

    stop() {
        this._removeMonitor();
        this.playing?.destroy();
        this.playing = null;
    }

    // A dock app icon (GNOME's draggable): what it was dragged from
    watchIcon(icon, dash) {
        const draggable = icon._draggable;
        if (!draggable)
            return;
        draggable.connectObject(
            'drag-begin', () => this._begin({kind: 'app', dash, app: icon.app, draggable}),
            'drag-cancelled', () => this._cancelled(draggable),
            'drag-end', () => this._ended(draggable), icon);
    }

    // A stack's or folder's button, made draggable here
    placeDraggable(button, dash, what) {
        button._delegate.getDragActor = () => what.picture(dash.iconSize);
        button._delegate.getDragActorSource = () => button.icon?.icon ?? button;
        const draggable = DND.makeDraggable(button, {timeoutThreshold: 200});
        draggable.connectObject(
            'drag-begin', () => this._begin({...what, dash, draggable}),
            'drag-cancelled', () => this._cancelled(draggable),
            'drag-end', () => this._ended(draggable), button);
        return draggable;
    }

    // An app in an open folder's card: dragged out of it, onto the desktop, it is unpinned
    memberDraggable(button, app, folder, popover) {
        button._delegate = {
            app,
            getDragActor: () => app.create_icon_texture(48),
            getDragActorSource: () => button,
        };
        const draggable = DND.makeDraggable(button, {timeoutThreshold: 200});
        draggable.connectObject(
            'drag-begin', () => {
                popover.close(true);
                this._begin({kind: 'member', dash: popover._dash, app, folder, draggable});
            },
            'drag-cancelled', () => this._cancelled(draggable),
            'drag-end', () => this._ended(draggable), button);
    }

    _begin(drag) {
        this._drag = drag;
        this._point = null;
        this._removeMonitor();
        this._monitor = {
            dragDrop: event => {
                const [x, y] = event.clutterEvent.get_coords();
                if (this._drag)
                    this._point = [x, y];
                return DND.DragDropResult.CONTINUE;
            },
        };
        DND.addDragMonitor(this._monitor);
        this._dock.dragChanged(true);
    }

    _removeMonitor() {
        if (this._monitor)
            DND.removeDragMonitor(this._monitor);
        this._monitor = null;
    }

    _ended(draggable) {
        if (this._drag?.draggable !== draggable)
            return;
        this._drag = null;
        this._point = null;
        this._removeMonitor();
        this._dock.dragChanged(false);
    }

    _cancelled(draggable) {
        const drag = this._drag;
        const point = this._point;
        if (drag?.draggable !== draggable)
            return;
        this._drag = null;
        this._point = null;
        this._removeMonitor();
        this._dock.dragChanged(false);
        // Escape: no point was let go of
        if (!point)
            return;
        const near = drag.kind === 'member' ? null : drag.dash.pillRect();
        if (near && inside(near, point[0], point[1], NEAR * scaleFactor()))
            return;
        if (!this._takeOut(drag))
            return;
        const actor = draggable._dragActor;
        if (actor)
            actor.visible = false;
        this._play(drag, point);
    }

    // Out of the dock; whether it was the dock's to take out
    _takeOut(drag) {
        const favorites = AppFavorites.getAppFavorites();
        const later = fn => global.compositor.get_laters().add(Meta.LaterType.BEFORE_REDRAW, () => {
            fn();
            return GLib.SOURCE_REMOVE;
        });
        if (drag.kind === 'app' || drag.kind === 'member') {
            const id = drag.app?.get_id();
            if (!id || !favorites.isFavorite(id))
                return false;
            later(() => favorites.removeFavorite(id));
            return true;
        }
        if (drag.kind === 'folder') {
            const ids = drag.folder.ids;
            later(() => {
                this._dock.folders?.takeApart(drag.folder);
                for (const id of ids)
                    favorites.removeFavorite(id);
            });
            return true;
        }
        if (drag.kind === 'stack') {
            later(() => this._dock.stacks?.remove(drag.place));
            return true;
        }
        return false;
    }

    _play(drag, [x, y]) {
        this.count++;
        const level = this._dock.motionLevel();
        const dash = drag.dash;
        const sf = scaleFactor();
        const size = dash.iconSize;
        const picture = drag.picture?.(size) ?? drag.app?.create_icon_texture(size);
        if (!picture)
            return;
        if (level === 'none') {
            picture.destroy();
            return;
        }
        this.playing?.destroy();
        const group = new St.Widget({layout_manager: new Clutter.FixedLayout(), x, y});
        this.playing = group;
        Main.layoutManager.uiGroup.add_child(group);
        group.connect('destroy', () => {
            if (this.playing === group)
                this.playing = null;
        });
        group.add_child(picture);
        const [, , w, h] = picture.get_preferred_size();
        picture.set_position(-w / 2, -h / 2);
        picture.set_pivot_point(0.5, 0.5);
        let left = 1;
        const done = () => {
            if (--left === 0)
                group.destroy();
        };
        if (level === 'reduced') {
            picture.ease({opacity: 0, duration: REDUCED_FADE, mode: EASE.out, onComplete: done});
            return;
        }
        picture.ease({
            scale_x: SHRINK.scale, scale_y: SHRINK.scale, opacity: 0,
            duration: SHRINK.duration, mode: EASE.out, onComplete: done,
        });
        const edge = dash.edgeName;
        SPECKS.forEach(([diameter, ox, oy], i) => {
            // Turned with the dock: away from its edge is up for a bottom dock, right for a left one
            const [dx, dy] = {left: [-oy, ox], right: [oy, ox]}[edge] ?? [ox, oy];
            const d = diameter * sf;
            const speck = new St.Widget({style_class: 'amethystora-dock-speck', width: d, height: d});
            speck.set_style(`border-radius: ${d / 2}px;`);
            speck.set_position(-d / 2, -d / 2);
            speck.set_pivot_point(0.5, 0.5);
            speck.opacity = 0;
            group.add_child(speck);
            left++;
            speck.ease({
                opacity: 255,
                translation_x: dx * SPECK.burstShare * sf,
                translation_y: dy * SPECK.burstShare * sf,
                delay: SPECK.stagger * i,
                duration: SPECK.burst,
                mode: EASE.out,
                onComplete: () => speck.ease({
                    opacity: 0,
                    translation_x: dx * sf,
                    translation_y: dy * sf,
                    scale_x: SPECK.end,
                    scale_y: SPECK.end,
                    duration: SPECK.fly,
                    mode: EASE.out,
                    onComplete: done,
                }),
            });
        });
    }
}
