// Named spaces in the overview (R5-32, board 22): the overview's strip of workspaces shows each one's name along its
// bottom edge and, under GNOME's own copies of its windows, the picture the workspace has of its own. The name is the
// one the top bar shows: the person's (GNOME's own list), else the tiling's, else, while names come from apps, the
// app with most windows there, else the workspace's number. The picture is the tiling's (workspace-backgrounds, or
// default-background), the same the desktop draws behind that workspace; a workspace without one keeps GNOME's look.
//
// GNOME makes the strip's thumbnails each time the overview opens and takes them away when it closes: this adds to
// each as it is made, by a wrap of ThumbnailsBox.addThumbnails (workspaceThumbnail.js, the same in GNOME 50 and 51),
// and listens to the names and pictures only while some thumbnail shows them. Nothing runs while the overview is shut.

import Clutter from 'gi://Clutter';
import Gio from 'gi://Gio';
import Meta from 'gi://Meta';
import Pango from 'gi://Pango';
import St from 'gi://St';

import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import * as WorkspaceThumbnail from 'resource:///org/gnome/shell/ui/workspaceThumbnail.js';

// GNOME's own list of names, by workspace position
const NAMES = 'workspace-names';

export class OverviewNames {
    // workspaces: the top bar's workspaces module (its GNOME names and its own settings); mainApp(workspace): the app
    // a workspace is named after while names come from apps
    constructor(workspaces, mainApp) {
        this._workspaces = workspaces;
        this._mainApp = mainApp;
    }

    enable() {
        this._shown = new Map();
        // The tiling's names and pictures, read through settings of its own: the tiling stops first and disposes its
        this._tilingSettings = this._workspaces._ext?.modules?.has('tiling')
            ? this._workspaces._ext.getSettings('org.gnome.shell.extensions.amethystora-ui.tiling') : null;
        const proto = WorkspaceThumbnail.ThumbnailsBox.prototype;
        this._original = proto.addThumbnails;
        const names = this;
        const original = this._original;
        proto.addThumbnails = function (...args) {
            const result = original.apply(this, args);
            try {
                names._decorateBox(this);
            } catch (e) {
                console.error(`Amethystora UI: the overview's workspace names: ${e.message}\n${e.stack}`);
            }
            return result;
        };
        // Started while the overview is open: what it shows already gets its names now
        for (const box of this._boxes())
            this._decorateBox(box);
    }

    disable() {
        if (this._original)
            WorkspaceThumbnail.ThumbnailsBox.prototype.addThumbnails = this._original;
        this._original = null;
        for (const thumbnail of [...this._shown?.keys() ?? []])
            this._undecorate(thumbnail);
        this._shown = null;
        this._unwatch();
        this._tilingSettings?.run_dispose();
        this._tilingSettings = null;
    }

    // The strips the overview shows now: the main screen's and each other screen's
    _boxes() {
        const controls = Main.overview._overview?._controls;
        const boxes = [controls?._thumbnailsBox];
        for (const view of controls?._workspacesDisplay?._workspacesViews ?? [])
            boxes.push(view?._thumbnails);
        return boxes.filter(box => box instanceof WorkspaceThumbnail.ThumbnailsBox);
    }

    _decorateBox(box) {
        for (const thumbnail of box._thumbnails ?? []) {
            if (!this._shown.has(thumbnail))
                this._decorate(thumbnail);
        }
    }

    _decorate(thumbnail) {
        const name = new St.Label({style_class: 'amethystora-workspaces-overview-name', reactive: false});
        name.clutter_text.use_markup = false;
        name.clutter_text.ellipsize = Pango.EllipsizeMode.END;
        name.clutter_text.line_alignment = Pango.Alignment.CENTER;
        name.add_constraint(new Clutter.BindConstraint({source: thumbnail, coordinate: Clutter.BindCoordinate.WIDTH}));
        name.add_constraint(new Clutter.AlignConstraint({source: thumbnail, align_axis: Clutter.AlignAxis.Y_AXIS, factor: 1}));
        thumbnail.add_child(name);
        const shown = {name, picture: null, key: null};
        this._shown.set(thumbnail, shown);
        thumbnail.connectObject('destroy', () => this._forget(thumbnail), this);
        this._watch();
        this._sync(thumbnail, shown);
    }

    _undecorate(thumbnail) {
        const shown = this._shown?.get(thumbnail);
        if (!shown)
            return;
        thumbnail.disconnectObject(this);
        shown.name.destroy();
        shown.picture?.destroy();
        this._shown.delete(thumbnail);
    }

    // GNOME took the thumbnail away (the overview closed): its actors went with it
    _forget(thumbnail) {
        this._shown?.delete(thumbnail);
        if (this._shown?.size === 0)
            this._unwatch();
    }

    // ---- What a workspace is called and has ----

    get _tiling() {
        return this._tilingSettings ?? null;
    }

    nameOf(workspace) {
        const index = workspace.index();
        const own = this._workspaces._prefs?.get_strv(NAMES)[index]?.trim() ||
            this._tiling?.get_strv('workspace-names')[index]?.trim();
        if (own)
            return own;
        const app = this._workspaces._settings?.get_boolean('auto-names') ? this._mainApp(workspace) : null;
        return app?.get_name() || String(index + 1);
    }

    // The workspace's own picture, as the tiling draws it, or null
    pictureOf(workspace) {
        const tiling = this._tiling;
        if (!tiling)
            return null;
        const uri = tiling.get_strv('workspace-backgrounds')[workspace.index()] || tiling.get_string('default-background');
        if (!uri)
            return null;
        const file = uri.includes('://') ? Gio.File.new_for_uri(uri) : Gio.File.new_for_path(uri);
        return file.query_exists(null) ? file : null;
    }

    _sync(thumbnail, shown) {
        const workspace = thumbnail.metaWorkspace;
        if (!workspace || workspace.index() < 0)
            return;
        shown.name.text = this.nameOf(workspace);
        const file = this.pictureOf(workspace);
        const key = file?.get_uri() ?? '';
        if (key === shown.key)
            return;
        shown.key = key;
        shown.picture?.destroy();
        shown.picture = null;
        if (!file)
            return;
        // Under the copies of the windows, as large as the thumbnail, covering it as the desktop's picture covers the
        // screen; drawn by St, which loads it as the tiling's backgrounds load the same file
        const uri = file.get_uri().replace(/["\\]/g, c => `\\${c}`);
        const picture = new St.Widget({
            style_class: 'amethystora-workspaces-overview-picture',
            style: `background-image: url("${uri}"); background-size: cover;`,
            reactive: false,
        });
        picture.add_constraint(new Clutter.BindConstraint({source: thumbnail, coordinate: Clutter.BindCoordinate.SIZE}));
        thumbnail.insert_child_below(picture, thumbnail._viewport ?? null);
        shown.picture = picture;
    }

    _syncAll() {
        for (const [thumbnail, shown] of this._shown ?? [])
            this._sync(thumbnail, shown);
    }

    // Listened to only while a thumbnail shows a name: what can change one, brought up to date once before the next frame
    _watch() {
        if (this._watching)
            return;
        this._watching = true;
        const queue = () => this._queue();
        this._workspaces._prefs?.connectObject(`changed::${NAMES}`, queue, this);
        this._workspaces._settings?.connectObject('changed::auto-names', queue, this);
        this._watchedTiling = this._tiling;
        this._watchedTiling?.connectObject(
            'changed::workspace-names', queue,
            'changed::workspace-backgrounds', queue,
            'changed::default-background', queue, this);
        global.workspace_manager.connectObject('workspaces-reordered', queue, this);
        global.display.connectObject('window-created', queue, this);
        global.window_manager.connectObject('destroy', queue, this);
    }

    _unwatch() {
        if (!this._watching)
            return;
        this._watching = false;
        this._workspaces._prefs?.disconnectObject(this);
        this._workspaces._settings?.disconnectObject(this);
        this._watchedTiling?.disconnectObject(this);
        this._watchedTiling = null;
        global.workspace_manager.disconnectObject(this);
        global.display.disconnectObject(this);
        global.window_manager.disconnectObject(this);
        if (this._later)
            global.compositor.get_laters().remove(this._later);
        this._later = 0;
    }

    _queue() {
        if (this._later)
            return;
        this._later = global.compositor.get_laters().add(Meta.LaterType.BEFORE_REDRAW, () => {
            this._later = 0;
            this._syncAll();
            return false;
        });
    }

    // For the suites: the names the strips show now, by workspace number
    names() {
        return [...this._shown?.entries() ?? []].map(([thumbnail, shown]) => ({
            index: thumbnail.metaWorkspace?.index() ?? -1, name: shown.name.text, picture: !!shown.picture,
        })).sort((a, b) => a.index - b.index);
    }
}
