// The dock's row of icons: GNOME's own dash, moved out of the overview. GNOME's code still lists the pinned and the
// running apps, sizes their icons to fit, shows their names, takes drops to pin and opens GNOME's app menu; this adds
// what the dock does beyond the dash: running apps kept in the order they started (on this screen's dock, those with
// a window on it), the last apps used, folders of pinned apps, stacks, the drives and the trash, a mark for a running
// app, the focused app lit, a wiggle for an app that wants attention, what apps say about themselves (a count and a
// progress bar, dock/badges.js), what a click does, the app grid button opening the overview, and the motion of the
// items under the pointer (dock/swell.js). The row runs along the dock's edge: across the bottom, or down a side.

import Clutter from 'gi://Clutter';
import GLib from 'gi://GLib';
import GObject from 'gi://GObject';
import Meta from 'gi://Meta';
import Mtk from 'gi://Mtk';
import Pango from 'gi://Pango';
import Shell from 'gi://Shell';
import St from 'gi://St';

import {gettext as _, ngettext} from 'resource:///org/gnome/shell/extensions/extension.js';
import * as AppFavorites from 'resource:///org/gnome/shell/ui/appFavorites.js';
import * as Dash from 'resource:///org/gnome/shell/ui/dash.js';
import * as DND from 'resource:///org/gnome/shell/ui/dnd.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import * as PopupMenu from 'resource:///org/gnome/shell/ui/popupMenu.js';

import {Badges, IconBadge} from './badges.js';
import {Clicks} from './clicks.js';
import {folderPicture} from './folders.js';
import {ItemMarks} from './marks.js';
import {PlaceButton, fillPlaceMenu} from './menus.js';
import {Previews} from './previews.js';
import {Swell} from './swell.js';
import {EASE, EDGES, scaleFactor} from './util.js';

// The largest icon. GNOME's next size up is 64, so a bound on the height between the two leaves 48 the largest.
const LARGEST = 48;
// In touch mode (modules/touch.js) GNOME's next size, for a fingertip
const LARGEST_TOUCH = 64;
// A call for attention: two shakes of up to 15 degrees at the start of every three seconds
const WIGGLE_EVERY = 3000;
const WIGGLE_TIME = 500;
const WIGGLE_ANGLE = 15;
// Milliseconds the pointer rests on an item of a row too long for the screen before it is scrolled into view
const HOVER_REVEAL = 100;
const SCROLL_TIME = 100;
// The room in a tile around its icon, on each side
const TILE_PADDING = 6;
// An item's name: this far beyond its far edge, shown as GNOME's are
const LABEL_GAP = 10;
const LABEL_SHOW = 150;
// Recent apps: their icons a sixth smaller, a little faded, and their caption over them while the pointer is there
const RECENT_SIZE = 5 / 6;
const RECENT_OPACITY = 235;
const CAPTION_ABOVE = 22;
const CAPTION_FADE = 160;
const POP = {from: 16, scale: 0.6, duration: 380};

// Most recently used first
const byUse = windows => windows.sort((a, b) => b.get_user_time() - a.get_user_time());

// An icon drawn at `drawn` px and shown at `size`: sharp where the dock grows it
function peakIcon(actor, size, drawn) {
    const sf = scaleFactor();
    const holder = new St.Widget({width: size * sf, height: size * sf, layout_manager: new Clutter.FixedLayout()});
    actor.set_pivot_point(0, 0);
    actor.set_scale(size / drawn, size / drawn);
    holder.add_child(actor);
    holder._amethystoraDrawn = drawn;
    return holder;
}

export const DockDash = GObject.registerClass(
class AmethystoraDockDash extends Dash.Dash {
    _init(dock, site) {
        super._init();
        this._dock = dock;
        this._site = site;
        this._places = dock.places;
        this._launcher = dock.launcher;
        this._badges = dock.badges;
        this._clicks = new Clicks(this, dock);
        this._items = new Map();
        this._placeItems = new Map();
        this._recentItems = new Map();
        this._dividers = [];
        this._attention = new Set();
        this._started = [];
        this.add_style_class_name('amethystora-dock-dash');
        this.add_style_class_name(`amethystora-dock-edge-${site.edgeName}`);
        // GNOME draws its dash into an offscreen buffer always, for the overview's fades; the dock's icons grow
        // beyond the row, and a buffer that size, made again as they grow, would be a copy every frame
        this.offscreen_redirect = Clutter.OffscreenRedirect.AUTOMATIC_FOR_OPACITY;
        const vertical = this.edge.vertical;
        this.set({
            x_align: {left: Clutter.ActorAlign.START, right: Clutter.ActorAlign.END}[site.edgeName] ??
                Clutter.ActorAlign.CENTER,
            y_align: vertical ? Clutter.ActorAlign.CENTER : Clutter.ActorAlign.END,
            reactive: true,
            track_hover: true,
        });

        // The dock's own entry in Ctrl+Alt+Tab, in place of the dash's
        Main.ctrlAltTabManager.removeGroup(this);
        Main.ctrlAltTabManager.addGroup(this, _('Dock'), 'user-bookmarks-symbolic');

        // GNOME lists running apps most recently used first, which would move them about as they are used: the
        // dock keeps them in the order they started, those with a window on its screen (and workspace, when it shows
        // only that one's), and leaves out an app whose every window shows a place
        const apps = Shell.AppSystem.get_default();
        this._appSystem = {get_running: () => this._inStartOrder(apps.get_running())};

        // A row too long for the screen scrolls, faded at the ends, but never under the wheel
        this._scroll = new St.ScrollView({
            style_class: 'amethystora-dock-scroll',
            hscrollbar_policy: vertical ? St.PolicyType.NEVER : St.PolicyType.EXTERNAL,
            vscrollbar_policy: vertical ? St.PolicyType.EXTERNAL : St.PolicyType.NEVER,
            enable_mouse_scrolling: false,
            x_expand: true,
            y_expand: true,
        });
        const orientation = vertical ? Clutter.Orientation.VERTICAL : Clutter.Orientation.HORIZONTAL;
        const row = new St.BoxLayout({x_expand: true, y_expand: true, orientation});
        this._dashContainer.remove_child(this._box);
        row.add_child(this._box);
        // GNOME's row claims no width of its own so that the overview's dash can shrink; inside the scroll view it
        // claims all of it, which is what the view scrolls through
        this._box.layout_manager = new Clutter.BoxLayout({orientation});
        this._scroll.child = row;
        this._dashContainer.insert_child_at_index(this._scroll, 0);
        this._dashContainer.add_style_class_name('amethystora-dock-row');
        this._dashContainer.orientation = orientation;
        if (vertical) {
            this._dashContainer.set({
                x_align: Clutter.ActorAlign.FILL, y_align: Clutter.ActorAlign.CENTER,
                x_expand: true, y_expand: false,
            });
            this._showAppsIcon.set({x_expand: false, y_expand: true, y_align: Clutter.ActorAlign.CENTER});
            // The pill is as wide as an icon and as long as the row
            const sizer = this._background.get_first_child();
            sizer?.clear_constraints();
            sizer?.add_constraint(new Clutter.BindConstraint({
                source: this._showAppsIcon.icon,
                coordinate: Clutter.BindCoordinate.WIDTH,
            }));
            sizer?.add_constraint(new Clutter.BindConstraint({
                source: this._dashContainer,
                coordinate: Clutter.BindCoordinate.HEIGHT,
            }));
        }

        // The app grid button opens the overview's app grid; it shows as on while the grid is shown
        const gridButton = Main.overview.dash.showAppsButton;
        this.showAppsButton.toggle_mode = false;
        this.showAppsButton.connectObject('clicked', () => this._toggleApps(), this);
        this._gridBinding = gridButton.bind_property('checked', this.showAppsButton, 'checked',
            GObject.BindingFlags.SYNC_CREATE);
        gridButton.connectObject('notify::checked', () => {
            if (!gridButton.checked)
                this._appsFromDesktop = false;
        }, this);
        Main.overview.connectObject('hidden', () => (this._appsFromDesktop = false), this);
        this._showAppsIcon.label?.add_style_class_name('amethystora-dock-label');
        this._wrapIcon(this._showAppsIcon.icon);
        this._showAppsIcon.icon._createIconTexture(this.iconSize);
        this._followLabel(this._showAppsIcon);

        St.Settings.get().connectObject('notify::enable-animations', () => this._syncWiggle(), this);
        global.display.connectObject(
            'notify::focus-window', () => this._onFocus(),
            'window-demands-attention', (display, window) => this._watchAttention(window),
            'window-marked-urgent', (display, window) => this._watchAttention(window),
            this);
        for (const window of global.display.list_all_windows()) {
            if (window.demands_attention || window.urgent)
                this._watchAttention(window);
        }
        this._places.connectObject(
            'places-changed', () => this._syncPlaces(),
            'place-changed', (o, place) => this._placeChanged(place),
            'windows-changed', () => {
                this._queueRedisplay();
                this._syncAll();
                this._queueGeometry();
            }, this);
        dock.stacks?.connectObject('changed', () => this._syncPlaces(),
            'place-changed', (o, place) => this._placeChanged(place),
            'arrived', (o, place) => this._arrived(place), this);
        dock.folders?.connectObject('changed', () => this._queueRedisplay(), this);
        dock.recent?.connectObject('changed', () => this._queueRedisplay(), this);
        // A count, a bar or an update changed, or notifications were silenced or let through again
        this._badges.connectObject('changed', () => this._syncAll(), this);
        Main.overview.connectObject(
            'item-drag-begin', () => this._dock.dragChanged(true),
            'item-drag-end', () => this._dock.dragChanged(false),
            'item-drag-cancelled', () => this._dock.dragChanged(false),
            'showing', () => this.swell.sync(),
            'shown', () => this.swell.sync(),
            'hiding', () => this.swell.sync(),
            'hidden', () => this.swell.sync(), this);
        this.connect('icon-size-changed', () => this._syncSizes());
        this.connect('notify::mapped', () => this._syncWiggle());
        // The caption is placed with the geometry, before the next frame: from inside the layout it asked for another
        this.connect('notify::allocation', () => this._queueGeometry());
        this.connect('notify::hover', () => {
            this._syncCaption();
            // A folder coloured in Files since: the stacks' own icons are read again as the pointer comes to the dock
            if (this.hover)
                this._dock.stacks?.reread();
        });
        this.connect('destroy', () => this._onDestroy());

        this.swell = new Swell(this, dock);
        this.previews = new Previews(this, dock);
        this._syncPlaces();
    }

    // ---- What the dock's parts ask ----

    get edge() {
        return EDGES[this._site.edgeName];
    }

    get edgeName() {
        return this._site.edgeName;
    }

    get monitorIndex() {
        return this._site.monitorIndex;
    }

    get reserves() {
        return this._site.reserves;
    }

    // The pill the glass is drawn behind
    get pill() {
        return this._background;
    }

    // What is drawn for an item and moves with the pointer: its button
    visualOf(container) {
        if (container === this._showAppsIcon)
            return this._showAppsIcon.toggleButton;
        return container?.child ?? null;
    }

    // Whether the dock is held at rest (spec 1.1)
    held() {
        return this._overflows() || this._menuOpen || this._dock.cardOpen || this._dock.dragging ||
            Main.overview.animationInProgress;
    }

    previewsHeld(state) {
        return Main.overview.visible || this._menuOpen || this._dock.cardOpen || this._dock.dragging ||
            Boolean(this._launcher.get(state.key?.get_id?.() ?? '')?.updating);
    }

    // The dock as it rests on the stage
    restRect() {
        const [x, y] = this.get_transformed_position();
        return {x, y, width: this.width, height: this.height};
    }

    // The pill where it rests: the box GNOME gave it, before the swell made it longer
    pillRect() {
        const [x, y] = this.get_transformed_position();
        const box = this._pillBox ?? this._background.get_allocation_box();
        return {x: x + box.x1, y: y + box.y1, width: box.get_width(), height: box.get_height()};
    }

    // The swell makes the pill longer by `grown` px and moves it `along` the row. GNOME gives the pill the dash's own
    // box, so it is given a longer one here, never stretched: its ends keep their curve and its edge its width. It is
    // allocated on its own, so that nothing else is laid out again while the dock moves
    growPill(grown, along) {
        // Even, so that both ends move by whole pixels
        grown = Math.max(0, Math.round(grown / 2) * 2);
        along = Math.round(along);
        if (this._pillGrown === grown && this._pillAlong === along)
            return;
        this._pillGrown = grown;
        this._pillAlong = along;
        this._allocatePill();
    }

    vfunc_allocate(box) {
        super.vfunc_allocate(box);
        this._pillBox = this._background.get_allocation_box();
        if (this._pillGrown || this._pillAlong)
            this._allocatePill();
    }

    _allocatePill() {
        if (!this._pillBox)
            return;
        // The box the pill was given is inside its margins, which allocating it takes off again
        const pill = this._background;
        const box = this._pillBox.copy();
        box.x1 -= pill.margin_left;
        box.x2 += pill.margin_right;
        box.y1 -= pill.margin_top;
        box.y2 += pill.margin_bottom;
        const half = (this._pillGrown ?? 0) / 2;
        const along = this._pillAlong ?? 0;
        if (this.edge.vertical) {
            box.y1 += along - half;
            box.y2 += along + half;
        } else {
            box.x1 += along - half;
            box.x2 += along + half;
        }
        pill.allocate(box);
    }

    stripRect() {
        return this._site.stripRect();
    }

    // An item's windows that are not kept from the taskbar, on this dock's screen (and workspace), most recently used
    // first
    windowsOf(key) {
        const windows = key instanceof Shell.App ? this._windowsOfApp(key) : this._places.windowsOf?.(key) ?? [];
        return byUse(windows.filter(window => this._shows(window)));
    }

    // Wherever they are
    allWindowsOf(key) {
        return byUse(key instanceof Shell.App ? this._windowsOfApp(key) : this._places.windowsOf?.(key) ?? []);
    }

    _shows(window) {
        if (this._dock.perScreen && window.get_monitor() !== this.monitorIndex)
            return false;
        if (this._dock.workspaceApps && !window.located_on_workspace(this._dock.shownOn(this.monitorIndex)))
            return false;
        return true;
    }

    // The app or place whose window has the focus
    focusedKey() {
        const window = global.display.focus_window;
        if (!window)
            return null;
        return this._places.owner(window) ?? Shell.WindowTracker.get_default().get_window_app(window);
    }

    clickApp(app) {
        this._clicks.act(this._clicks.appTargetFor(app), Clutter.BUTTON_PRIMARY);
    }

    openFan(state) {
        return this._dock.fan?.show(this, state) ?? false;
    }

    closePopups() {
        this.previews?.hide(true);
    }

    // The widest the row may be. GNOME picks the largest of its icon sizes that fits both ways; the bound across
    // leaves 48 px the largest, whatever padding the theme puts around an icon. What is around the icon is measured
    // against the icon as drawn, not this.iconSize: in the overview GNOME eases an icon to its new size, and every
    // frame of that changes the dock's size and comes back here, which against this.iconSize gave a new limit, a new
    // size and a new ease, for ever.
    fitTo(length) {
        const scale = scaleFactor();
        const vertical = this.edge.vertical;
        const [, across] = vertical ? this.get_preferred_width(-1) : this.get_preferred_height(-1);
        const icon = this._showAppsIcon.icon?.icon;
        let drawn = this.iconSize * scale;
        if (icon)
            [, drawn] = vertical ? icon.get_preferred_width(-1) : icon.get_preferred_height(-1);
        const around = Math.max(0, across - drawn);
        // What GNOME does not count: the pill's own ends and the lines between the groups
        const pill = this._background.get_theme_node();
        const ends = (vertical ? pill.get_vertical_padding() + pill.get_border_width(St.Side.TOP) * 2
            : pill.get_horizontal_padding() + pill.get_border_width(St.Side.LEFT) * 2) + 8 * scale * 3;
        // GNOME takes the dash's top and bottom margins off the bound across the row; on a side dock those centre it
        // along the row instead, and are given back
        const along = vertical ? this.margin_top + this.margin_bottom : 0;
        const largest = this._dock.touchMode ? LARGEST_TOUCH : Math.max(LARGEST, this._dock.iconSize ?? LARGEST);
        this.setMaxSize(Math.max(0, length - ends), around + (largest + 4) * scale + along);
    }

    // The size a person picked (R6-28, icon-size, 40 to 64 px), as large as the row's length lets every item be; with
    // none picked, or in touch mode, GNOME's own choice among its sizes. The arithmetic is the row's: its length, less
    // what each item takes around its picture and the gaps between them, shared out.
    _adjustIconSize() {
        const wanted = this._dock.touchMode ? null : this._dock.iconSize;
        if (!wanted) {
            super._adjustIconSize();
            return;
        }
        if (this._maxWidth === -1 || this._maxHeight === -1)
            return;
        const items = this._box.get_children().filter(actor => actor.child?._delegate?.icon && !actor.animatingOut);
        items.push(this._showAppsIcon);
        const button = items[0].child;
        const icon = button?._delegate?.icon;
        if (!icon?.icon)
            return;
        icon.icon.ensure_style();
        const [, , pictureWidth] = icon.icon.get_preferred_size();
        const [, , buttonWidth] = button.get_preferred_size();
        const node = this.get_theme_node();
        const gaps = (items.length - 1) * node.get_length('spacing');
        const room = this._maxWidth - node.get_horizontal_padding() - items.length * (buttonWidth - pictureWidth) - gaps;
        const fits = Math.floor(room / items.length / scaleFactor());
        const size = Math.max(16, Math.min(wanted, fits));
        if (size === this.iconSize)
            return;
        this.iconSize = size;
        for (const item of items)
            item.child._delegate.icon.setIconSize(size);
        // After the sizes, so that what follows the signal (the recent apps' smaller icons) has the last word
        this.emit('icon-size-changed');
    }

    vfunc_scroll_event() {
        return Clutter.EVENT_STOP;
    }

    _onDestroy() {
        this.swell?.destroy();
        this.previews?.destroy();
        this._clicks.destroy();
        this._gridBinding?.unbind();
        this._gridBinding = null;
        // GNOME's dash leaves its label timers running when it goes, which then show a label that is gone: the
        // dock goes whenever the screen locks, and the pointer is often over it
        for (const id of [this._wiggleId, this._revealId, this._showLabelTimeoutId, this._resetHoverTimeoutId]) {
            if (id)
                GLib.source_remove(id);
        }
        this._wiggleId = this._revealId = this._showLabelTimeoutId = this._resetHoverTimeoutId = 0;
        this._wiggle?.stop();
        this._wiggle = null;
        if (this._geometryId)
            global.compositor.get_laters().remove(this._geometryId);
        this._geometryId = 0;
        for (const window of this._attention)
            window.disconnectObject(this);
        this._attention.clear();
        this._dock.stacks?.disconnectObject(this);
        this._dock.folders?.disconnectObject(this);
        this._dock.recent?.disconnectObject(this);
        this._caption?.destroy();
        this._caption = null;
        for (const item of this._recentItems.values()) {
            if (!item.get_parent())
                item.destroy();
        }
        this._recentItems.clear();
        for (const state of this._items.values())
            state.marks?.destroy();
        // The minimize animation would otherwise still aim at where the dock was
        for (const window of global.display.list_all_windows())
            window.set_icon_geometry(null);
    }

    // ---- The row ----

    // GNOME redraws the pinned and running apps; the dock then puts its folders, recent apps and lines among them.
    // The recent apps are taken out while GNOME redraws, since GNOME counts every app in the row as its own.
    _redisplay() {
        for (const item of this._recentItems.values()) {
            if (item.get_parent() === this._box)
                this._box.remove_child(item);
        }
        for (const divider of this._dividers)
            divider.destroy();
        this._dividers = [];
        this._separator?.destroy();
        this._separator = null;
        // GNOME only lets items grow and shrink in the overview; on the desktop it is the dock's to do (_createAppItem)
        this._onDesktop = this._shownInitially && !Main.overview.visible;
        try {
            super._redisplay();
        } finally {
            this._onDesktop = false;
        }
        // GNOME's line between pinned and running apps gives way to the dock's own lines
        this._separator?.destroy();
        this._separator = null;

        this._syncFolders();
        this._syncRecent();
        this._syncDividers();
        this._syncAll();
        this._queueGeometry();
        this.swell?.invalidate();
        this._dock.rowChanged(this);
    }

    _isPinned(app) {
        return AppFavorites.getAppFavorites().isFavorite(app.get_id());
    }

    _inStartOrder(running) {
        const primary = this.monitorIndex === Main.layoutManager.primaryIndex || !this._dock.perScreen;
        running = running.filter(app => app.get_n_windows() === 0 ? primary : this._windowsOfApp(app)
            .some(window => this._shows(window)));
        this._started = this._started.filter(app => running.includes(app));
        for (const app of running) {
            if (!this._started.includes(app))
                this._started.push(app);
        }
        return [...this._started];
    }

    // The running apps this dock lists
    runningApps() {
        return this._started;
    }

    // ...less the shelf's window, which is a place to drop things rather than an app (modules/shelf.js)
    _windowsOfApp(app) {
        const shelf = this._dock.ext.modules?.get('shelf');
        return app.get_windows().filter(window => !window.skip_taskbar && !this._places.owner(window) &&
            !shelf?.owns?.(window));
    }

    // An app's icon is GNOME's; what the dock does with it is given to that one icon
    _createAppItem(app) {
        const item = super._createAppItem(app);
        const icon = item.child;
        icon.activate = button => this._clicks.act(this._clicks.appTarget(icon), button);
        // The dock's own sign that an app starts, in place of GNOME's zoom (spec 1.3); GNOME's Ctrl or middle click
        // and the menu's New Window come here too
        icon.animateLaunch = () => this._dock.launch?.begin(app);
        // The item grows in and shrinks away on the desktop too
        item.show = animate => Dash.DashItemContainer.prototype.show.call(item, animate || this._onDesktop);
        item.destroy = () => {
            if (this._onDesktop && !item.animatingOut) {
                delete item.destroy;
                item.animateOutAndDestroy();
            } else {
                Dash.DashItemContainer.prototype.destroy.call(item);
            }
        };
        // The icon being dragged stays behind, faint, rather than at full strength
        icon.scaleAndFade = () => (icon.opacity = 51);
        icon.undoScaleAndFade = () => (icon.opacity = 255);
        icon.getDragActor = () => app.create_icon_texture(this.iconSize);
        // GNOME's single running dot gives way to the dock's mark
        icon._dot.opacity = 0;
        // GNOME leaves the menu behind when the icon goes
        icon.connect('destroy', () => icon._menu?.destroy());
        // A right click or a long press fans the app's windows out, when it has some here
        icon.popupMenu = () => {
            if (this.openFan(this._items.get(icon)))
                return false;
            return Dash.DashIcon.prototype.popupMenu.call(icon);
        };
        // A pinned app takes another dropped on its middle, to make a folder; a window's files go to the phone
        icon.handleDragOver = (source, actor, x, y) => this._iconDragOver(icon, source, x, y);
        icon.acceptDrop = source => this._iconAcceptDrop(icon, source);
        this._wrapIcon(icon.icon);
        icon.icon._createIconTexture(this.iconSize);
        this._followLabel(item);
        this._decorate(item, app);
        this._dock.dissolve?.watchIcon(icon, this);
        return item;
    }

    // The icon is made at the size the dock grows it to and shown at its own
    _wrapIcon(baseIcon) {
        if (baseIcon._amethystoraCreate)
            return;
        const create = baseIcon.createIcon;
        baseIcon._amethystoraCreate = create;
        baseIcon.createIcon = size => {
            const peak = this._dock.peak;
            const actor = create.call(baseIcon, peak > 1 ? Math.round(size * peak) : size);
            return peak > 1 ? peakIcon(actor, size, Math.round(size * peak)) : actor;
        };
    }

    // Icons made again at a new size the dock grows them to
    reloadIcons() {
        for (const state of this._items.values())
            state.button.icon?._createIconTexture?.(state.button.icon.iconSize);
        this._showAppsIcon.icon._createIconTexture(this._showAppsIcon.icon.iconSize);
        this.swell?.invalidate();
    }

    _iconDragOver(icon, source, x, y) {
        if (source === Main.xdndHandler) {
            const state = this._items.get(icon);
            if (state && icon.app.get_id() === this._dock.phoneId)
                return this._dock.drops?.overPhone(this, state) ?? DND.DragMotionResult.CONTINUE;
            return DND.DragMotionResult.CONTINUE;
        }
        if (!this._isPinned(icon.app) || !this._dock.folders)
            return DND.DragMotionResult.CONTINUE;
        return this._dock.folders.over(this, {actor: icon, visual: icon, id: icon.app.get_id()}, source, x, y);
    }

    _iconAcceptDrop(icon, source) {
        if (!this._dock.folders)
            return false;
        return this._dock.folders.accept({actor: icon, id: icon.app.get_id()}, source);
    }

    // A window's files over the dock away from its items; GNOME's own drop of an app otherwise. A side dock gives
    // GNOME's placeholder a position along its length.
    handleDragOver(source, actor, x, y, time) {
        if (source === Main.xdndHandler)
            return this._dock.drops?.overDock(this) ?? DND.DragMotionResult.CONTINUE;
        if (this.edge.vertical && this._box.height > 0)
            x = y * this._box.width / this._box.height;
        return super.handleDragOver(source, actor, x, y, time);
    }

    // ---- Places, stacks and folders ----

    _placeChanged(place) {
        const item = this._placeItems.get(place);
        item?.child.sync();
        item?.setLabelText(place.name);
    }

    // Something arrived in the Downloads stack, or it was opened (R5-35): the count on its tile, and, for a new
    // arrival, a short hop of its icon, the newest put forward for a moment
    _arrived(place) {
        const item = this._placeItems.get(place);
        const state = item ? this._items.get(item.child) : null;
        if (!state)
            return;
        const before = state.arrivedShown ?? 0;
        this._sync(state);
        // The picture's holder, as an app starting hops (launch.js): the item itself is the swell's to move
        const bin = item.child.icon?._iconBin;
        if ((place.arrived ?? 0) > before && bin && St.Settings.get().enable_animations &&
            !this._dock.ext?.modules?.get('comfort')?.reduceMotion) {
            const along = this.edge.short === 'y' ? 'translation_y' : 'translation_x';
            bin.remove_transition(along === 'translation_y' ? 'translation-y' : 'translation-x');
            bin.ease({
                [along]: this.edge.away * 8 * scaleFactor(),
                duration: 160,
                mode: Clutter.AnimationMode.EASE_OUT_QUAD,
                onStopped: () => bin.ease({[along]: 0, duration: 260, mode: Clutter.AnimationMode.EASE_OUT_BOUNCE}),
            });
        }
    }

    _syncPlaces() {
        const wanted = [...this._dock.stacks?.list() ?? [], ...this._places.list()];
        for (const [place, item] of [...this._placeItems]) {
            if (wanted.includes(place))
                continue;
            this._placeItems.delete(place);
            // As the apps' items do: never while the overview opens or closes
            if (Main.overview.animationInProgress)
                item.destroy();
            else
                item.animateOutAndDestroy();
        }
        wanted.forEach((place, index) => {
            if (this._placeItems.has(place)) {
                // In the order wanted
                const item = this._placeItems.get(place);
                const next = wanted.slice(index + 1).map(other => this._placeItems.get(other)).find(Boolean);
                if (next && item.get_parent() === this._box)
                    this._box.set_child_below_sibling(item, next);
                return;
            }
            const item = new Dash.DashItemContainer();
            const button = new PlaceButton(place,
                mouseButton => place.stack ? this._dock.openCard(this, this._items.get(button))
                    : this._clicks.act(this._clicks.placeTarget(place), mouseButton),
                menu => place.stack ? this._fillStackMenu(menu, place)
                    : fillPlaceMenu(menu, place, this._places, () => this.windowsOf(place)));
            button.connect('menu-state-changed', (o, open) => this._itemMenuStateChanged(item, open));
            item.setChild(button);
            item.setLabelText(place.name);
            this._wrapIcon(button.icon);
            button.icon.setIconSize(this.iconSize);
            this._hookUpLabel(item);
            this._followLabel(item);
            // Before the next place already there (the trash stays last)
            const next = wanted.slice(index + 1).map(other => this._placeItems.get(other)).find(Boolean);
            const at = next ? this._box.get_children().indexOf(next) : -1;
            this._box.insert_child_at_index(item, at >= 0 ? at : this._box.get_n_children());
            this._placeItems.set(place, item);
            this._decorate(item, place);
            if (place.stack) {
                button._delegate.handleDragOver = source => source === Main.xdndHandler && this._dock.drops
                    ? this._dock.drops.overStack(this, this._items.get(button)) : DND.DragMotionResult.CONTINUE;
                this._dock.dissolve?.placeDraggable(button, this, {
                    kind: 'stack', place, picture: size => new St.Icon({gicon: place.gicon, icon_size: size}),
                });
            }
            item.show(this._shownInitially && !Main.overview.animationInProgress);
        });
        // The lines and the icon size, with the places counted
        this._queueRedisplay();
    }

    _fillStackMenu(menu, place) {
        menu.removeAll();
        const heading = new PopupMenu.PopupMenuItem(place.name, {reactive: false, can_focus: false});
        heading.label.clutter_text.use_markup = false;
        heading.add_style_class_name('amethystora-dock-menu-heading');
        menu.addMenuItem(heading);
        menu.addAction(_('Open in Files'), () => {
            this._dock.stacks.open(place);
            Main.overview.hide();
        });
        menu.addAction(_('Remove from the dock'), () => this._dock.stacks.remove(place));
    }

    // A folder stands where its first app stands, the others' items hidden; GNOME still counts them all
    _syncFolders() {
        const folders = this._dock.folders?.list ?? [];
        const children = this._box.get_children().filter(child => child._amethystoraApp ?? child.child?._delegate?.app);
        for (const container of children) {
            const app = container._amethystoraApp ?? container.child._delegate.app;
            const id = app.get_id();
            const folder = folders.find(f => f.ids.includes(id));
            const first = folder?.ids[0] === id;
            if (folder && first) {
                this._makeFolder(container, app, folder);
            } else {
                this._unmakeFolder(container);
                container.visible = !folder || container.animatingOut;
            }
        }
    }

    _makeFolder(container, app, folder) {
        const button = container._amethystoraFolder;
        const apps = folder.ids.map(id => Shell.AppSystem.get_default().lookup_app(id)).filter(Boolean);
        if (button && button.place.folder.name === folder.name &&
            button.place.folder.ids.join() === folder.ids.join()) {
            container.visible = true;
            return;
        }
        this._unmakeFolder(container);
        container.hideLabel();
        const place = {
            folder,
            name: folder.name,
            ownPeak: true,
            createIcon: size => {
                const peak = this._dock.peak;
                return folderPicture(apps, size, peak > 1 ? Math.round(size * peak) : size);
            },
        };
        const folderButton = new PlaceButton(place,
            () => this._dock.openCard(this, this._items.get(folderButton)),
            menu => this._dock.folders.fillMenu(menu, folder, this, this._items.get(folderButton)));
        // GNOME's diff and drop take the folder for its first app
        folderButton.app = app;
        folderButton.connect('menu-state-changed', (o, open) => this._itemMenuStateChanged(container, open));
        folderButton._delegate.handleDragOver = (source, actor, x, y) => this._dock.folders.over(this,
            {actor: folderButton, visual: folderButton, id: app.get_id(), folder}, source, x, y);
        folderButton._delegate.acceptDrop = source => this._dock.folders.accept(
            {actor: folderButton, id: app.get_id(), folder}, source);
        const icon = container.child;
        // The app's own item rests under the folder, unmarked
        const hidden = this._items.get(icon);
        if (hidden) {
            hidden.underFolder = true;
            hidden.marks?.sync(this._dock.indicator, 0);
            if (hidden.marks)
                hidden.marks.layer.visible = false;
        }
        container._amethystoraApp = app;
        container._amethystoraIcon = icon;
        // The app's own icon, kept aside, goes with its container (GNOME's redraw, the dock made again)
        if (!container._amethystoraKeeps) {
            container._amethystoraKeeps = true;
            container.connect('destroy', () => {
                const kept = container._amethystoraIcon;
                container._amethystoraIcon = null;
                if (kept && !kept.get_parent())
                    kept.destroy();
            });
        }
        container.remove_child(icon);
        container.child = folderButton;
        container.insert_child_at_index(folderButton, 0);
        container.setLabelText(folder.name);
        folderButton.icon.setIconSize(this.iconSize);
        container._amethystoraFolder = folderButton;
        this._hookUpLabel(container);
        this._decorate(container, place);
        this._dock.dissolve?.placeDraggable(folderButton, this, {
            kind: 'folder', folder, picture: size => folderPicture(apps, size, size),
        });
        container.visible = true;
        this._pop(folderButton);
    }

    _unmakeFolder(container) {
        const button = container._amethystoraFolder;
        if (!button)
            return;
        const icon = container._amethystoraIcon;
        container.hideLabel();
        container._amethystoraFolder = null;
        container._amethystoraIcon = null;
        container._amethystoraApp = null;
        this._items.get(button)?.marks?.destroy();
        this._items.delete(button);
        button.destroy();
        container.child = icon;
        container.insert_child_at_index(icon, 0);
        container.setLabelText(icon.app.get_name());
        const state = this._items.get(icon);
        if (state) {
            state.underFolder = false;
            if (state.marks)
                state.marks.layer.visible = true;
            this._sync(state);
        }
    }

    // ---- Recent apps ----

    _syncRecent() {
        const recent = this._dock.recent;
        const apps = recent ? recent.shownFor(this._started) : [];
        const fresh = !this._recentFilled;
        this._recentFilled = true;
        for (const [app, item] of [...this._recentItems]) {
            if (apps.includes(app))
                continue;
            this._recentItems.delete(app);
            if (item.get_parent() !== this._box)
                item.destroy();
            else if (this._shownInitially && !Main.overview.animationInProgress)
                item.animateOutAndDestroy();
            else
                item.destroy();
        }
        // After the running apps, before the places
        const firstPlace = this._box.get_children().find(child => child.child instanceof PlaceButton &&
            !child.child.place.folder);
        apps.forEach(app => {
            let item = this._recentItems.get(app);
            const isNew = !item;
            if (!item) {
                item = this._createAppItem(app);
                item._amethystoraRecent = true;
                item.child.opacity = RECENT_OPACITY;
                item.child.icon.setIconSize(Math.round(this.iconSize * RECENT_SIZE));
                this._recentItems.set(app, item);
            }
            if (item.get_parent() !== this._box) {
                if (firstPlace)
                    this._box.insert_child_below(item, firstPlace);
                else
                    this._box.add_child(item);
            } else if (firstPlace) {
                this._box.set_child_below_sibling(item, firstPlace);
            }
            if (isNew) {
                item.show(false);
                if (!fresh && apps[0] === app)
                    this._pop(item.child);
            }
        });
        // Newest first
        for (let i = apps.length - 1; i > 0; i--) {
            const item = this._recentItems.get(apps[i]);
            const before = this._recentItems.get(apps[i - 1]);
            if (item?.get_parent() === this._box && before?.get_parent() === this._box)
                this._box.set_child_above_sibling(item, before);
        }
        this._placeCaption();
    }

    // The board's pop: from nearer the edge, small and clear, to its place
    _pop(actor) {
        const level = this._dock.motionLevel();
        if (level === 'none' || !this._shownInitially)
            return;
        actor._amethystoraBusy = true;
        const sf = scaleFactor();
        const toEdge = POP.from * sf * -this.edge.away;
        const short = this.edge.short === 'y' ? 'translation_y' : 'translation_x';
        const opacity = actor.opacity || 255;
        const rest = {scale_x: 1, scale_y: 1, opacity, [short]: 0};
        // Not onComplete: _redisplay takes a recent item out of the row, which ends its pop unfinished, and an icon left
        // where it stopped stays clear and small in its slot, seen by nothing else, for as long as the app is recent
        const onStopped = () => {
            actor.set(rest);
            actor._amethystoraBusy = false;
        };
        if (level === 'reduced') {
            actor.opacity = 0;
            actor.ease({opacity, duration: 150, mode: EASE.out, onStopped});
            return;
        }
        actor.set({scale_x: POP.scale, scale_y: POP.scale, opacity: 0, [short]: toEdge});
        actor.ease({...rest, duration: POP.duration, mode: EASE.back, onStopped});
    }

    // The caption over the recent apps (the board's: its top 22 px above their icons, at the group's start), placed
    // as the pointer comes onto the dock, when everything has its place, and shown only while it is there
    _placeCaption() {
        const items = [...this._recentItems.values()].filter(item => item.get_parent() === this._box &&
            item.visible && item.has_allocation());
        if (!items.length || !this._dock.recent) {
            this._caption?.hide();
            return false;
        }
        if (!this._caption) {
            this._caption = new St.Label({style_class: 'amethystora-dock-recent-caption', opacity: 0});
            this._caption.clutter_text.use_markup = false;
            // One short word, never cut short
            this._caption.clutter_text.ellipsize = Pango.EllipsizeMode.NONE;
            Main.layoutManager.addChrome(this._caption);
        }
        this._caption.text = _('Recent').toLocaleUpperCase();
        const icons = items.map(item => this._items.get(item.child)).filter(Boolean)
            .map(state => this.iconRestRect(state)).filter(Boolean);
        if (!icons.length)
            return false;
        const sf = scaleFactor();
        const [, , width, height] = this._caption.get_preferred_size();
        const rtl = this.get_text_direction() === Clutter.TextDirection.RTL;
        const x1 = Math.min(...icons.map(r => r.x));
        const x2 = Math.max(...icons.map(r => r.x + r.width));
        const y1 = Math.min(...icons.map(r => r.y));
        let x, y;
        if (this.edgeName === 'bottom') {
            x = rtl ? x2 - width : x1;
            y = y1 - CAPTION_ABOVE * sf;
        } else {
            y = y1 - height - 6 * sf;
            x = this.edgeName === 'left' ? x1 : x2 - width;
        }
        this._caption.set_position(Math.round(x), Math.round(y));
        this._caption.show();
        return true;
    }

    _syncCaption() {
        if (this.hover && !this._placeCaption())
            return;
        const caption = this._caption;
        if (!caption?.visible)
            return;
        caption.ease({opacity: this.hover ? 255 : 0, duration: CAPTION_FADE, mode: EASE.out});
    }

    // The lines between the groups that have something in them: pinned apps and folders, running apps, recent
    // apps, then stacks, drives and the trash
    _syncDividers() {
        const children = this._box.get_children().filter(child => child.child && !child.animatingOut &&
            child.visible);
        const group = child => {
            if (child._amethystoraRecent)
                return 2;
            if (child.child instanceof PlaceButton && !child.child.place.folder)
                return 3;
            const app = child._amethystoraApp ?? child.child?._delegate?.app;
            if (child.child instanceof PlaceButton || app && this._isPinned(app))
                return 0;
            return 1;
        };
        let last = null;
        for (const child of children) {
            const kind = group(child);
            if (last !== null && kind !== last) {
                const vertical = this.edge.vertical;
                const divider = new St.Widget({
                    style_class: 'dash-separator amethystora-dock-divider',
                    x_align: vertical ? Clutter.ActorAlign.CENTER : Clutter.ActorAlign.START,
                    y_align: vertical ? Clutter.ActorAlign.START : Clutter.ActorAlign.CENTER,
                });
                if (vertical)
                    divider.width = this.iconSize;
                else
                    divider.height = this.iconSize;
                this._box.insert_child_below(divider, child);
                this._dividers.push(divider);
            }
            last = kind;
        }
    }

    // ---- Labels ----

    // The name beyond the item's far edge as it is drawn, following it as it grows (spec 1.2)
    _followLabel(item) {
        // A label going away no longer follows the item, so that the swell reads nothing of it
        const hide = item.hideLabel.bind(item);
        item.hideLabel = () => {
            item._amethystoraLabelRest = null;
            hide();
        };
        item.showLabel = () => {
            const label = item.label;
            if (!item._labelText || !label)
                return;
            label.set_text(item._labelText);
            label.clutter_text.use_markup = false;
            label.opacity = 0;
            label.show();
            const rest = this.restRectOf(item);
            const sf = scaleFactor();
            const [, , width, height] = label.get_preferred_size();
            let x, y;
            if (this.edgeName === 'bottom') {
                x = Math.clamp(rest.x + (rest.width - width) / 2, 0, global.stage.width - width);
                y = rest.y - LABEL_GAP * sf - height;
            } else if (this.edgeName === 'left') {
                x = rest.x + rest.width + LABEL_GAP * sf;
                y = rest.y + (rest.height - height) / 2;
            } else {
                x = rest.x - LABEL_GAP * sf - width;
                y = rest.y + (rest.height - height) / 2;
            }
            label.set_position(Math.round(x), Math.round(y));
            item._amethystoraLabelRest = rest;
            this._labelFollow(item);
            label.ease({opacity: 255, duration: LABEL_SHOW, mode: EASE.out});
        };
    }

    _labelFollow(item) {
        const rest = item._amethystoraLabelRest;
        if (!rest)
            return;
        const label = item.label;
        if (!label)
            return;
        const shown = item._amethystoraShown ?? {scale: 1, along: 0, away: 0};
        const vertical = this.edge.vertical;
        const across = vertical ? rest.width : rest.height;
        const out = across * (shown.scale - 1) + shown.away;
        if (this.edgeName === 'bottom')
            label.set({translation_x: shown.along, translation_y: -out});
        else
            label.set({translation_y: shown.along, translation_x: this.edgeName === 'left' ? out : -out});
    }

    // The item's button where it rests on the stage: from its container, which nothing transforms
    restRectOf(container) {
        const visual = this.visualOf(container) ?? container;
        const [x, y] = container.get_transformed_position();
        const box = visual === container ? null : visual.get_allocation_box();
        return box
            ? {x: x + box.x1, y: y + box.y1, width: box.get_width(), height: box.get_height()}
            : {x, y, width: container.width, height: container.height};
    }

    // Each frame of the swell: the marks and labels follow the items along the row
    followSwell(items) {
        const long = this.edge.long;
        let recentMoved = false;
        for (const item of items) {
            const container = item.container;
            container._amethystoraShown = item.shown;
            const state = this._items.get(item.actor);
            state?.marks?.follow(item.shown.along, long);
            this._labelFollow(container);
            // GNOME's menu reads where its icon is only when it is laid out: the swollen icon going back to rest as its
            // menu opens would otherwise leave the menu high, until something lays it out and it drops
            if (item.actor._menu?.isOpen)
                item.actor._menu.actor.queue_relayout();
            recentMoved ||= Boolean(container._amethystoraRecent);
        }
        // The caption rises with the recent apps as they grow, so that they never cover it
        if (recentMoved && this._caption?.visible) {
            let out = 0;
            let along = null;
            for (const item of this._recentItems.values()) {
                const shown = item._amethystoraShown;
                if (!shown || item.get_parent() !== this._box)
                    continue;
                const rest = this.restRectOf(item);
                const across = this.edge.vertical ? rest.width : rest.height;
                out = Math.max(out, across * (shown.scale - 1) + shown.away);
                // The group's start is the newest's near edge, which moves with it and with its growth
                const length = this.edge.vertical ? rest.height : rest.width;
                const rtl = !this.edge.vertical && this.get_text_direction() === Clutter.TextDirection.RTL;
                along ??= shown.along + (rtl ? 1 : -1) * length * (shown.scale - 1) / 2;
            }
            if (this.edgeName === 'bottom')
                this._caption.set_translation(along ?? 0, -out, 0);
            else
                this._caption.set_translation(0, along ?? 0, 0);
        }
    }

    // ---- An item's decorations ----

    // The mark, the count, the progress bar and the urgency of one item, whether app, place, stack or folder
    _decorate(item, key) {
        const button = item.child;
        const container = button._iconContainer;
        const dash = this;
        const state = {
            item,
            key,
            button,
            dash,
            badge: key instanceof Shell.App && container ? new IconBadge(container, button.icon, TILE_PADDING) : null,
            restCentre() {
                const rect = dash.restRectOf(item);
                return dash.edge.long === 'x' ? rect.x + rect.width / 2 : rect.y + rect.height / 2;
            },
            restRect() {
                return dash.restRectOf(item);
            },
            name() {
                return key instanceof Shell.App ? key.get_name() : key.name;
            },
        };
        state.marks = new ItemMarks(state);
        item.label?.add_style_class_name('amethystora-dock-label');
        // The Downloads stack's count of what arrived since it was last opened (R5-35), in the corner an app's
        // count has
        if (key.stack && key.value === 'xdg:DOWNLOAD' && container) {
            state.arrivals = new St.Label({
                style_class: 'amethystora-dock-badge amethystora-dock-arrivals',
                x_align: Clutter.ActorAlign.END,
                y_align: Clutter.ActorAlign.START,
                x_expand: true,
                y_expand: true,
                visible: false,
            });
            container.add_child(state.arrivals);
        }

        // A row too long for the screen shows an item the pointer rests on, or the keyboard reaches
        button.connect('notify::hover', () => {
            this._onHover(button, item);
            this.previews?.hovered(state);
        });
        button.connect('key-focus-in', () => this._reveal(item));
        button.connect('clicked', () => this._reveal(item));
        button.connect('destroy', () => {
            state.marks?.destroy();
            state.marks = null;
            this._items.delete(button);
        });
        if (key instanceof Shell.App) {
            key.connectObject('windows-changed', () => {
                this._sync(state);
                this._queueGeometry();
            }, button);
        }
        this._items.set(button, state);
        this._syncSize(state);
        this._sync(state);
        this._dock.launch?.adopt(state);
    }

    _onFocus() {
        this._syncAll();
        const focused = this.focusedKey();
        for (const state of this._items.values()) {
            if (state.key === focused)
                this._reveal(state.item);
        }
    }

    _syncAll() {
        for (const state of this._items.values())
            this._sync(state);
        this._syncWiggle();
    }

    _sync(state) {
        const {key, button} = state;
        if (state.underFolder)
            return;
        if (key.folder || key.stack) {
            state.marks?.sync(this._dock.indicator, 0);
            if (state.arrivals) {
                const count = key.arrived ?? 0;
                state.arrivedShown = count;
                state.arrivals.visible = count > 0;
                // "+3": a count, which the language writes
                state.arrivals.text = count > 0 ? `+${count.toLocaleString()}` : '';
                button.accessible_name = count > 0
                    ? ngettext('%s, %d new', '%s, %d new', count).format(key.name, count) : key.name;
            }
            return;
        }
        const windows = this.windowsOf(key);
        const app = key instanceof Shell.App;
        const entry = app ? this._launcher.get(key.get_id()) : null;

        const focused = key === this.focusedKey();
        state.marks?.focus(focused, !this.has_style_class_name('amethystora-dock-swells'));
        state.marks?.sync(this._dock.indicator, windows.length);

        if (focused)
            button.add_style_class_name('amethystora-dock-focused');
        else
            button.remove_style_class_name('amethystora-dock-focused');

        const count = app ? this._badges.unread(key) : 0;
        if (state.badge?.sync(count, entry))
            button.accessible_name = Badges.accessibleName(app ? key.get_name() : key.name, count, app ? key : null);

        const urgent = Boolean(entry?.urgent) || windows.some(window => this._attention.has(window));
        if (urgent && !state.urgent)
            this._reveal(state.item);
        state.urgent = urgent;
    }

    // Every mark made again (the indicator changed)
    remark() {
        for (const state of this._items.values())
            this._sync(state);
    }

    _syncSizes() {
        for (const state of this._items.values())
            this._syncSize(state);
        for (const divider of this._dividers) {
            if (this.edge.vertical)
                divider.width = this.iconSize;
            else
                divider.height = this.iconSize;
        }
        // GNOME gives every icon the row's size; a recent app's is smaller
        for (const item of this._recentItems.values())
            item.child?.icon.setIconSize(Math.round(this.iconSize * RECENT_SIZE));
        this.swell?.invalidate();
    }

    // The count and the bar scale with the icon
    _syncSize(state) {
        state.badge?.resize(this.iconSize);
        state.marks?.place();
    }

    _watchAttention(window) {
        if (this._attention.has(window))
            return;
        this._attention.add(window);
        const forget = () => {
            window.disconnectObject(this);
            this._attention.delete(window);
            this._syncAll();
        };
        window.connectObject(
            'notify::demands-attention', () => !window.demands_attention && !window.urgent && forget(),
            'notify::urgent', () => !window.demands_attention && !window.urgent && forget(),
            'unmanaged', forget,
            this);
        this._syncAll();
    }

    // One clock for every icon that wiggles, so that they move together; none while animations are off, while
    // notifications are silenced or while the dock is not on screen
    _syncWiggle() {
        const shaking = [...this._items.values()].some(state => state.urgent);
        const run = shaking && this.mapped && St.Settings.get().enable_animations && !this._launcher.quiet;
        if (run && !this._wiggleId) {
            this._shake();
            this._wiggleId = GLib.timeout_add(GLib.PRIORITY_DEFAULT, WIGGLE_EVERY, () => {
                this._shake();
                return GLib.SOURCE_CONTINUE;
            });
        } else if (!run && this._wiggleId) {
            GLib.source_remove(this._wiggleId);
            this._wiggleId = 0;
            this._wiggle?.stop();
            this._tilt(0);
        } else if (run) {
            // An icon that stopped asking stands still at once
            this._tilt(null);
        }
    }

    _shake() {
        if (!this._wiggle) {
            this._wiggle = new Clutter.Timeline({actor: this, duration: WIGGLE_TIME});
            this._wiggle.connect('new-frame', timeline =>
                this._tilt(WIGGLE_ANGLE * Math.sin(4 * Math.PI * timeline.get_progress())));
            this._wiggle.connect('completed', () => this._tilt(0));
        }
        // As fast as GNOME's own animations are
        this._wiggle.duration = Math.max(1, WIGGLE_TIME * St.Settings.get().slow_down_factor);
        this._wiggle.rewind();
        this._wiggle.start();
    }

    // The angle for every icon that wants attention, 0 for the rest; null keeps the shaking ones as they are
    _tilt(angle) {
        for (const state of this._items.values()) {
            const image = state.button.icon?.icon;
            if (!image)
                continue;
            if (!state.urgent || angle === 0) {
                image.rotation_angle_z = 0;
            } else if (angle !== null) {
                image.set_pivot_point(0.5, 0.5);
                image.rotation_angle_z = angle;
            }
        }
    }

    // Each window minimizes into its item, where the item rests
    _queueGeometry() {
        if (this._geometryId)
            return;
        this._geometryId = global.compositor.get_laters().add(Meta.LaterType.BEFORE_REDRAW, () => {
            this._geometryId = 0;
            this._placeCaption();
            for (const state of this._items.values()) {
                if (!state.button.mapped || state.key.folder || state.key.stack)
                    continue;
                const rest = this.restRectOf(state.item);
                const rect = new Mtk.Rectangle({
                    x: Math.round(rest.x),
                    y: Math.round(rest.y),
                    width: Math.round(rest.width),
                    height: Math.round(rest.height),
                });
                for (const window of this.windowsOf(state.key))
                    window.set_icon_geometry(rect);
            }
            return GLib.SOURCE_REMOVE;
        });
    }

    // Where an item's icon picture rests on the stage, for the motion module
    iconRestRect(state) {
        const icon = state.button.icon?.icon;
        if (!icon?.has_allocation() || !state.item.visible)
            return null;
        const [x, y] = state.item.get_transformed_position();
        let [ix, iy] = [0, 0];
        for (let actor = icon; actor && actor !== state.item; actor = actor.get_parent()) {
            const box = actor.get_allocation_box();
            ix += box.x1;
            iy += box.y1;
        }
        return {x: x + ix, y: y + iy, width: icon.width, height: icon.height};
    }

    _overflows() {
        const adjustment = this.edge.vertical ? this._scroll.vadjustment : this._scroll.hadjustment;
        const [, , upper, , , page] = adjustment.get_values();
        return upper > page;
    }

    _itemMenuStateChanged(item, opened) {
        super._itemMenuStateChanged(item, opened);
        this._menuOpen = opened;
        this.swell?.sync();
        if (opened)
            this.previews?.hide(true);
        this._dock.menuChanged(this, opened);
    }

    // actor is pointed at, or no longer; item is what is scrolled into view
    _onHover(actor, item) {
        if (this._revealId)
            GLib.source_remove(this._revealId);
        this._revealId = 0;
        if (!actor.hover || !this._overflows())
            return;
        this._revealId = GLib.timeout_add(GLib.PRIORITY_DEFAULT, HOVER_REVEAL, () => {
            this._revealId = 0;
            this._reveal(item);
            return GLib.SOURCE_REMOVE;
        });
    }

    // Scrolls a row too long for the screen until the item is clear of the faded ends
    _reveal(item) {
        if (!this._overflows() || !item.has_allocation() || item.get_parent() !== this._box)
            return;
        const vertical = this.edge.vertical;
        const adjustment = vertical ? this._scroll.vadjustment : this._scroll.hadjustment;
        const [value, , upper, , , page] = adjustment.get_values();
        const fade = this._scroll.get_theme_node().get_length(vertical ? '-st-vfade-offset' : '-st-hfade-offset');
        const box = item.get_allocation_box();
        const rtl = !vertical && this._box.get_text_direction() === Clutter.TextDirection.RTL;
        const [b1, b2] = vertical ? [box.y1, box.y2] : [box.x1, box.x2];
        const start = rtl ? upper - b2 : b1;
        const end = rtl ? upper - b1 : b2;
        let target = value;
        if (start < value + fade)
            target = start - fade;
        else if (end > value + page - fade)
            target = end - page + fade;
        target = Math.clamp(target, 0, upper - page);
        if (target !== value)
            adjustment.ease(target, {duration: SCROLL_TIME, mode: Clutter.AnimationMode.EASE_OUT_QUAD});
    }

    // From the desktop the button opens the app grid, and closes it again; in the overview it switches between
    // the windows and the app grid, as GNOME's own button does
    _toggleApps() {
        const gridButton = Main.overview.dash.showAppsButton;
        if (!Main.overview.visible) {
            Main.overview.showApps();
            this._appsFromDesktop = true;
        } else if (gridButton.checked && this._appsFromDesktop) {
            Main.overview.hide();
        } else {
            gridButton.checked = !gridButton.checked;
        }
    }
});

