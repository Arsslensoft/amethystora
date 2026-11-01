// The app grid opens on the apps used most: a row of them under the label "Recently used", then "All apps" over GNOME's
// own grid, whose folders fill themselves from the apps' categories (the image's defaults in org.gnome.desktop.app-folders).
// The row is GNOME's own app icons, so a click, a drag and the right-click menu are GNOME's; what it holds comes from
// GNOME's own record of use, read only as the overview opens. An account that has used nothing yet sees no row and no
// labels. The folders are drawn as the image draws them: a tile with its four apps in a square, and the open folder a
// pane of glass with its name over its apps.

import Clutter from 'gi://Clutter';
import GLib from 'gi://GLib';
import Graphene from 'gi://Graphene';
import Meta from 'gi://Meta';
import Pango from 'gi://Pango';
import Shell from 'gi://Shell';
import St from 'gi://St';

import * as AppDisplay from 'resource:///org/gnome/shell/ui/appDisplay.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import * as ParentalControlsManager from 'resource:///org/gnome/shell/misc/parentalControlsManager.js';
import {InjectionManager, gettext as _} from 'resource:///org/gnome/shell/extensions/extension.js';

const MOST = 8;
// GNOME's grid fits three rows to a page of any wide shape, so a short screen's icons shrink to make room for the row:
// with the row, a page as wide as that may take two rows instead
const TWO_ROWS = {rows: 2, columns: 8};
const WAIT_FRAMES = 30;
// How many times one opening of the overview lets the row follow the grid to another shape: the row's height is part
// of the room the grid picks it for, so the two could take turns for ever
const REGROWS = 3;
// How long the grid's icons have to keep their size before the two are held to one size (ms)
const SETTLE = 300;
// The folder tile's square, as a share of its side: the corners, the inset of the four apps and the gap between them
const TILE_RADIUS = 18 / 64;
const TILE_INSET = 9 / 64;
const TILE_GAP = 5 / 64;
const TILE_CLASS = 'amethystora-appgrid-tile';
const ICON_CLASS = 'amethystora-appgrid-folder-icon';
const GROUP_CLASS = 'amethystora-appgrid';

export class AppGrid {
    constructor(ext) {
        this._ext = ext;
    }

    enable() {
        this._settings = this._ext.getSettings('org.gnome.shell.extensions.amethystora-ui.appgrid');
        this._appDisplay = Main.overview._overview?.controls?._appDisplay ?? null;
        if (!this._appDisplay?._box || !this._appDisplay._grid) {
            console.warn('Amethystora UI: appgrid: GNOME\'s app grid is not where it was, so it is left as it is');
            this._appDisplay = null;
            return;
        }
        // The theme's colours for the folders apply only while this runs
        Main.layoutManager.overviewGroup.add_style_class_name(GROUP_CLASS);
        this._folders();
        this._fullPages();
        this._settings.connectObject('changed::recent', () => this._sync(), this);
        this._sync();
    }

    disable() {
        this._settings?.disconnectObject(this);
        this._removeRow();
        this._unFullPages();
        this._unfolders();
        Main.layoutManager.overviewGroup.remove_style_class_name(GROUP_CLASS);
        this._appDisplay = null;
        // Disposed now rather than by the collector (dock.js says why)
        this._settings?.run_dispose();
        this._settings = null;
    }

    _sync() {
        if (this._settings.get_boolean('recent'))
            this._addRow();
        else
            this._removeRow();
    }

    // --- The recent row ----------------------------------------------------------------------------------------------

    _addRow() {
        if (this._row)
            return;
        const box = this._appDisplay._box;
        this._recentLabel = this._label(_('Recently used'));
        this._row = new St.BoxLayout({style_class: 'amethystora-appgrid-row', x_align: Clutter.ActorAlign.START});
        this._allLabel = this._label(_('All apps'));
        box.insert_child_at_index(this._recentLabel, 0);
        box.insert_child_at_index(this._row, 1);
        box.insert_child_at_index(this._allLabel, 2);
        this._icons = new Map();
        this._alignAgain = 0;
        this._alignWaits = 0;
        this._show(false);
        const grid = this._appDisplay._grid;
        this._gridModes = grid._gridModes;
        if (Array.isArray(this._gridModes))
            grid.setGridModes([...this._gridModes, TWO_ROWS]);
        // Filled as the overview opens, and only then
        Main.overview.connectObject('showing', () => {
            this._regrows = 0;
            this._alignAgain = 0;
            this._alignWaits = 0;
            this._fill();
        },
            // Once the overview has opened, where the grid came to rest: its icons' size then is the row's (the
            // grid is smaller for a moment as the overview opens)
            'shown', () => {
                this._rowSize = 0;
                this._alignAgain = 0;
                this._queueAlign();
            }, this);
        // The grid lays itself out again when the screen, the row or its pages change: the row follows it then, once the
        // layout is over (sizes changed while the grid is being laid out leave its icons without an allocation, and the
        // grid then draws over its old frames)
        this._appDisplay._grid.connectObject('notify::allocation', () => this._queueAlign(), this);
        // The grid's rows and columns settle only as it is laid out: a shape picked after the overview began to open is
        // a new number of apps for the row and a new size for its icons, which may be larger
        this._appDisplay._grid.layout_manager.connectObject('notify::icon-size', () => this._iconSizeChanged(),
            'notify::columns-per-page', () => this._shapeChanged(),
            'notify::rows-per-page', () => this._shapeChanged(), this);
        Main.layoutManager.connectObject('monitors-changed', () => {
            this._rowSize = 0;
            this._unpin();
        }, this);
        if (Main.overview.visible)
            this._fill();
    }

    _removeRow() {
        if (this._settle)
            GLib.source_remove(this._settle);
        this._settle = 0;
        if (this._alignLater)
            global.compositor.get_laters().remove(this._alignLater);
        this._alignLater = 0;
        this._first?.disconnectObject(this);
        this._first = null;
        this._unpin();
        Main.layoutManager.disconnectObject(this);
        Main.overview.disconnectObject(this);
        this._appDisplay?._grid.disconnectObject(this);
        this._appDisplay?._grid.layout_manager.disconnectObject(this);
        if (this._gridModes)
            this._appDisplay?._grid.setGridModes(this._gridModes);
        this._gridModes = null;
        for (const name of ['_recentLabel', '_row', '_allLabel']) {
            this[name]?.destroy();
            this[name] = null;
        }
        this._icons = null;
    }

    _label(text) {
        const label = new St.Label({
            style_class: 'amethystora-appgrid-label',
            text: text.toLocaleUpperCase(),
            x_align: Clutter.ActorAlign.START,
        });
        label.clutter_text.use_markup = false;
        // St does not count the letter spacing in the label's width
        label.clutter_text.ellipsize = Pango.EllipsizeMode.NONE;
        return label;
    }

    _show(shown) {
        for (const actor of [this._recentLabel, this._row, this._allLabel])
            actor.visible = shown;
    }

    // The apps used most, most first, that are installed, shown in the grid and allowed by Parental Controls
    _apps(count) {
        const system = Shell.AppSystem.get_default();
        const parental = ParentalControlsManager.getDefault();
        const apps = [];
        for (const used of Shell.AppUsage.get_default().get_most_used()) {
            const app = system.lookup_app(used.get_id());
            const info = app?.app_info;
            if (!info || !info.should_show() || !parental.shouldShowApp(info))
                continue;
            apps.push(app);
            if (apps.length >= count)
                break;
        }
        return apps;
    }

    _fill() {
        const layout = this._appDisplay._grid.layout_manager;
        const count = Math.min(MOST, layout.columnsPerPage || MOST);
        const apps = this._apps(count);
        const wanted = new Set(apps.map(app => app.get_id()));
        for (const [id, icon] of this._icons) {
            if (!wanted.has(id)) {
                icon.destroy();
                this._icons.delete(id);
            }
        }
        apps.forEach((app, index) => {
            let icon = this._icons.get(app.get_id());
            if (!icon) {
                // Not draggable: GNOME's grid moves only its own icons, and one of the row's dropped on it would
                // throw inside its drag and drop
                icon = new AppDisplay.AppIcon(app, {isDraggable: false, expandTitleOnHover: false});
                this._icons.set(app.get_id(), icon);
                this._row.add_child(icon);
            }
            this._row.set_child_at_index(icon, index);
        });
        this._align();
        // Shown or hidden only when that changes, so that the grid lays its pages out again only then
        if (this._row.visible !== apps.length > 0)
            this._show(apps.length > 0);
    }

    // Whether the row may take a new size from the grid once more in this opening
    _regrow() {
        if (!Main.overview.visible || (this._regrows ?? 0) >= REGROWS)
            return false;
        this._regrows = (this._regrows ?? 0) + 1;
        this._rowSize = 0;
        this._alignAgain = 0;
        this._unpin();
        return true;
    }

    _shapeChanged() {
        this._refill = this._regrow();
        this._queueAlign();
    }

    _iconSizeChanged() {
        this._queueAlign();
        if (this._settle)
            GLib.source_remove(this._settle);
        this._settle = GLib.timeout_add(GLib.PRIORITY_DEFAULT, SETTLE, () => {
            this._settle = 0;
            this._pin();
            return GLib.SOURCE_REMOVE;
        });
    }

    // On a short screen no one size fits by itself: with the row at the grid's size the grid has room only for smaller
    // icons, and with the row at those it has room for the larger ones again. The grid is then held at the row's
    // size, which fits, until the screen or the grid's shape changes.
    _pin() {
        const layout = this._appDisplay._grid.layout_manager;
        if (this._rowSize > 0 && layout.fixedIconSize <= 0 && layout.iconSize > this._rowSize) {
            layout.fixedIconSize = this._rowSize;
            this._pinned = true;
            this._pickIconSize(layout);
        }
    }

    _unpin() {
        if (this._pinned && this._appDisplay) {
            const layout = this._appDisplay._grid.layout_manager;
            layout.fixedIconSize = -1;
            this._pickIconSize(layout);
        }
        this._pinned = false;
    }

    // GNOME's grid picks its icons' size only when its page changes size, whatever fixedIconSize says
    _pickIconSize(layout) {
        const {_pageWidth: width, _pageHeight: height} = layout;
        layout.adaptToSize(width, height - 1);
        layout.adaptToSize(width, height);
    }

    _queueAlign() {
        if ((this._alignAgain ?? 0) >= 5)
            return;
        this._alignLater ||= global.compositor.get_laters().add(Meta.LaterType.BEFORE_REDRAW, () => {
            this._alignLater = 0;
            if (this._refill) {
                this._refill = false;
                this._fill();
            } else {
                this._align();
            }
            return false;
        });
    }

    // The row in the grid's own columns and at its icon size: as far from the left and as far apart as the grid's first
    // row. Done again only when the grid itself changes (its size, or the size of its icons), never per frame.
    _align() {
        if (!this._row || !this._icons?.size)
            return;
        // What it changes lays the grid out again, and the grid then picks its icons' size for the room left: the row
        // looks once more after that, until nothing changes (a few times at most). Its icons' size changes its height,
        // which changes the grid's: within one opening of the overview it only gets smaller, or the two would take
        // turns at each other's size for ever
        let changed = false;
        const layout = this._appDisplay._grid.layout_manager;
        const grid = layout.fixedIconSize > 0 ? layout.fixedIconSize : layout.iconSize;
        const size = this._rowSize > 0 ? Math.min(grid, this._rowSize) : grid;
        if (size > 0)
            this._rowSize = size;
        const [first, second] = (this._appDisplay._orderedItems ?? []).filter(item => item.visible);
        const placed = first?.has_allocation() && first.width > 0 && this._appDisplay._scrollView.has_allocation();
        // The grid's own size stays as its icons change size and place: its first icon is followed instead
        if (first !== this._first) {
            this._first?.disconnectObject(this);
            this._first = first ?? null;
            first?.connectObject('notify::allocation', () => this._queueAlign(),
                'destroy', () => (this._first = null), this);
        }
        for (const icon of this._icons.values()) {
            if (size > 0 && icon.icon.iconSize !== size) {
                icon.icon.setIconSize(size);
                changed = true;
            }
            if (placed && icon.width !== first.width) {
                icon.set_width(first.width);
                changed = true;
            }
        }
        if (placed) {
            changed = this._alignRow(first, second, layout) || changed;
            this._alignWaits = 0;
        }
        this._alignAgain = changed ? (this._alignAgain ?? 0) + 1 : 0;
        // Adding the row takes its allocation from the first icon until the next layout, which gives it the same one
        // back and so tells nobody: the row looks again at the next frames, for half a second at most
        const waiting = !placed && first && Main.overview.visible && this._alignWaits++ < WAIT_FRAMES;
        if (changed || waiting)
            this._queueAlign();
    }

    // The row as far from the left and as far apart as the grid's first row; whether that changed anything
    _alignRow(first, second, layout) {
        // Within the app grid, whatever the overview does to it as it opens; a later page is further right
        const box = this._appDisplay._box;
        const at = item => item.apply_relative_transform_to_point(box, new Graphene.Point3D({x: 0, y: 0, z: 0})).x;
        const left = at(first);
        // One page, as the box can hold all of them for a moment
        const page = this._appDisplay._scrollView.width;
        if (left < 0 || left >= page)
            return false;
        let spacing = layout.columnSpacing ?? 0;
        if (second?.has_allocation()) {
            const step = Math.abs(at(second) - left);
            if (step > first.width)
                spacing = step - first.width;
        }
        // Right to left, the grid starts at the right and so do the labels and the row. St scales a style's pixels.
        const rtl = box.get_text_direction() === Clutter.TextDirection.RTL;
        const {scale_factor: scale} = St.ThemeContext.get_for_stage(global.stage);
        const inset = rtl ? page - left - first.width : left;
        const pad = `padding-${rtl ? 'right' : 'left'}: ${Math.round(inset / scale)}px;`;
        const rowStyle = `${pad} spacing: ${Math.round(spacing / scale)}px;`;
        // Set only when it changes: a new style lays the grid out again
        if (this._row.get_style() === rowStyle)
            return false;
        this._row.set_style(rowStyle);
        this._recentLabel.set_style(pad);
        this._allLabel.set_style(pad);
        return true;
    }

    // --- The pages ------------------------------------------------------------------------------------------------------

    // A second page only once the first is full. GNOME keeps each page as the account's saved layout has it, however much
    // room is left, and puts a new app on the second page; the order of the apps stays the account's, the pages do not.
    // Its folders' grids already work this way: the setting below fills a page as an app leaves it and offers no new page
    // to drop on, and what GNOME places from the saved layout is packed here afterwards.
    _fullPages() {
        const display = this._appDisplay;
        const layout = display._grid.layout_manager;
        layout.allow_incomplete_pages = false;
        const fill = () => this._fillPages();
        this._injections.overrideMethod(AppDisplay.AppDisplay.prototype, '_redisplay', original => function (...args) {
            original.apply(this, args);
            fill();
        });
        // The grid picks its rows and columns as it is laid out, for the room it has: packed once that is over
        this._pageSizeIds = ['notify::rows-per-page', 'notify::columns-per-page'].map(name =>
            layout.connect(name, () => {
                this._fillLater ||= global.compositor.get_laters().add(Meta.LaterType.BEFORE_REDRAW, () => {
                    this._fillLater = 0;
                    this._fillPages();
                    return false;
                });
            }));
        this._fillPages();
    }

    _unFullPages() {
        if (this._fillLater)
            global.compositor.get_laters().remove(this._fillLater);
        this._fillLater = 0;
        const layout = this._appDisplay?._grid.layout_manager;
        for (const id of this._pageSizeIds ?? [])
            layout?.disconnect(id);
        this._pageSizeIds = null;
        if (layout)
            layout.allow_incomplete_pages = true;
    }

    // Each page as full as the grid's page size allows, the order kept; GNOME moves nothing when that size changes. Not
    // during a drag, whose drop target is a place in the pages as they are. Saved only when something moved.
    _fillPages() {
        const display = this._appDisplay;
        if (!display || display._dragMonitor)
            return;
        const grid = display._grid;
        const visible = page => grid.getItemsAtPage(page).filter(item => item.visible);
        let moved = false;
        for (let page = 0; page < grid.nPages; page++) {
            const items = visible(page);
            for (let i = items.length - 1; i >= grid.itemsPerPage; i--) {
                grid.moveItem(items[i], page + 1, 0);
                moved = true;
            }
            for (let room = grid.itemsPerPage - items.length; room > 0 && page < grid.nPages - 1; room--) {
                const next = visible(page + 1)[0];
                if (!next)
                    break;
                grid.moveItem(next, page, -1);
                moved = true;
            }
        }
        if (moved)
            display._savePages();
    }

    // --- The folders -------------------------------------------------------------------------------------------------

    // GNOME draws a folder's four apps in a square of its own making; the image's tile is drawn round them
    _folders() {
        this._injections = new InjectionManager();
        this._injections.overrideMethod(AppDisplay.FolderView.prototype, 'createFolderIcon', original => function (size, ...rest) {
            const icon = original.call(this, size, ...rest);
            try {
                shapeFolder(icon, size);
                if (rest[0] instanceof AppDisplay.FolderIcon)
                    rest[0].add_style_class_name(TILE_CLASS);
            } catch (e) {
                console.error(`Amethystora UI: appgrid: a folder's tile: ${e.message}`);
            }
            return icon;
        });
        this._redrawFolders();
    }

    _unfolders() {
        this._injections?.clear();
        this._injections = null;
        this._redrawFolders();
    }

    // The folders already drawn, drawn again by whichever createFolderIcon is in place now
    _redrawFolders() {
        for (const item of this._appDisplay?._orderedItems ?? []) {
            if (!(item instanceof AppDisplay.FolderIcon))
                continue;
            item.remove_style_class_name(TILE_CLASS);
            item.icon?.update?.();
        }
    }
}

// The square: rounded, its four apps set in from its edges and apart by the image's measures, the whole as large as
// GNOME made it
function shapeFolder(icon, size) {
    const inset = Math.round(size * TILE_INSET);
    const gap = Math.round(size * TILE_GAP);
    const sub = Math.floor((size - 2 * inset - gap) / 2);
    icon.add_style_class_name(ICON_CLASS);
    icon.set_style(`width: ${size - 2 * inset}px; height: ${size - 2 * inset}px; padding: ${inset}px; ` +
        `border-radius: ${Math.round(size * TILE_RADIUS)}px;`);
    const layout = icon.layout_manager;
    if (layout instanceof Clutter.GridLayout) {
        layout.row_spacing = gap;
        layout.column_spacing = gap;
    }
    for (const bin of icon.get_children()) {
        bin.set_style(`width: ${sub}px; height: ${sub}px;`);
        if (bin.child instanceof St.Icon)
            bin.child.icon_size = sub;
    }
}
