// The share guard: while the whole screen is shared, the windows of chosen apps are covered, blurred or with a solid
// card, from before their first frame, for everyone who sees the share and on this screen alike (Mutter cannot leave
// one window out of a picture of the screen, so the cover says what it is). A window shared on its own is never
// covered; nor is anything while only a recording runs. A click held on a cover takes it away until the share ends,
// unless someone else controls this computer through the share.
//
// Mutter hands the shell a handle for every share as it starts (the remote access controller's new-handle, which
// GNOME's own sharing indicator follows). The guard covers at once, and lifts the covers only when Mutter's screen cast
// service says that every stream is a window's (streams.js): until it has answered, and whenever it cannot, the screen
// counts as shared. Nothing else runs while nothing is shared. The covers are children of the windows' own actors, so
// that they follow the windows everywhere, in the overview's and the switcher's pictures of them too.

import Clutter from 'gi://Clutter';
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import Meta from 'gi://Meta';
import Shell from 'gi://Shell';
import St from 'gi://St';

import {gettext as _} from 'resource:///org/gnome/shell/extensions/extension.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';

import {plain} from '../pill/text.js';
import {windowsOnly} from './streams.js';

// The blur: strong enough that no word of a window's text can be read at 100 % scale
const BLUR_RADIUS = 40;
const BLUR_BRIGHTNESS = 0.85;
const BLUR = 'amethystora-privacy-blur';
// How long a click is held to uncover a window
const HOLD_MS = 600;
const ICON_SIZE = 34;
// How long the pill says what is covered, in seconds
const NOTICE_SECONDS = 4;
// A share that stopped while others go on is asked about again once, this much later, when Mutter has let its streams go
const SETTLE_MS = 1000;
const FILES = 'org.gnome.Nautilus.desktop';
// A menu, a list or a tooltip of a covered app is covered with no words of its own
const SMALL = new Set([Meta.WindowType.DROPDOWN_MENU, Meta.WindowType.POPUP_MENU, Meta.WindowType.TOOLTIP,
    Meta.WindowType.COMBO]);

// Whether uri is the place itself or somewhere inside it
const inside = (uri, root) => uri === root || uri.startsWith(`${root}/`);

// The actor that draws what the app drew: Wayland's surface container or a surface actor; Mutter's black background
// for a fullscreen window that does not fill its screen is not it
function surfaceOf(actor, cover) {
    const children = actor.get_children().filter(child => child !== cover);
    // Mutter's surface classes are not in its introspection data: they are told by their type's name
    return children.find(child => /Surface(Container)?Actor/.test(child.constructor?.name ?? '')) ??
        children.at(-1) ?? null;
}

// One window's cover, inside the window's actor over its frame (the actor also holds the shadow). It is the solid
// card until the blur is on the window's picture and has been drawn once; then, in blur, a tint over the blur.
class Cover {
    constructor(actor, app, look, uncover) {
        this.actor = actor;
        this.window = actor.get_meta_window();
        this.app = app;
        this.widget = new St.Widget({
            style_class: 'amethystora-privacy-cover amethystora-privacy-cover-opaque',
            reactive: true,
            layout_manager: new Clutter.BinLayout(),
            accessible_name: _('Hidden while you share'),
        });
        if (!SMALL.has(this.window.get_window_type())) {
            const column = new St.BoxLayout({
                style_class: 'amethystora-privacy-cover-column',
                orientation: Clutter.Orientation.VERTICAL,
                x_align: Clutter.ActorAlign.CENTER,
                y_align: Clutter.ActorAlign.CENTER,
            });
            const icon = app?.create_icon_texture(ICON_SIZE) ??
                new St.Icon({icon_name: 'dialog-password-symbolic', icon_size: ICON_SIZE});
            icon.x_align = Clutter.ActorAlign.CENTER;
            column.add_child(icon);
            const title = new St.Label({style_class: 'amethystora-privacy-cover-title', text: _('Hidden while you share'),
                x_align: Clutter.ActorAlign.CENTER});
            const line = new St.Label({style_class: 'amethystora-privacy-cover-line', text: _('Only you can uncover it'),
                x_align: Clutter.ActorAlign.CENTER});
            for (const label of [title, line]) {
                label.clutter_text.use_markup = false;
                column.add_child(label);
            }
            this.widget.add_child(column);
        }
        // Gone with the window's actor, when the window closes
        this.widget.connect('destroy', () => {
            this.widget = null;
        });

        const hold = new Clutter.LongPressGesture({long_press_duration_ms: HOLD_MS});
        hold.connect('recognize', () => uncover(this.window));
        this.widget.add_action(hold);

        actor.add_child(this.widget);
        this.window.connectObject(
            'size-changed', () => this._place(),
            'position-changed', () => this._place(),
            this);
        actor.connectObject(
            // A window covered as it opens has its size from its first frame
            'first-frame', () => this._place(),
            // An X11 window's surface is added when it is associated: the cover stays above it, and the blur follows it
            'child-added', (a, child) => this._childAdded(child),
            'child-removed', (a, child) => {
                if (child === this._blurred)
                    this._reblur();
            },
            this);
        this._place();
        this.setLook(look);
    }

    // Over the frame, which sits inside the buffer the actor is the size of
    _place() {
        if (!this.widget)
            return;
        const frame = this.window.get_frame_rect();
        const buffer = this.window.get_buffer_rect();
        this.widget.set_position(frame.x - buffer.x, frame.y - buffer.y);
        this.widget.set_size(Math.max(frame.width, 1), Math.max(frame.height, 1));
        this._blur();
    }

    _childAdded(child) {
        if (!this.widget || child === this.widget)
            return;
        this.actor.set_child_above_sibling(this.widget, null);
        if (this.look === 'blur' && surfaceOf(this.actor, this.widget) !== this._blurred)
            this._reblur();
    }

    // Blur: GNOME's blur on the window's own picture (the surface the app draws, with its popups), under the cover,
    // which stays sharp; one pass while the window is drawn. Card: the cover's own fill, and no blur.
    setLook(look) {
        if (look === this.look || !this.widget)
            return;
        this.look = look;
        this._unblur();
        this.widget.remove_style_class_name('amethystora-privacy-cover-blur');
        this.widget.remove_style_class_name('amethystora-privacy-cover-card');
        this.widget.add_style_class_name(`amethystora-privacy-cover-${look}`);
        if (look === 'blur')
            this._blur();
    }

    // Only once the window has drawn something: a blur of nothing has no size to be made at. The cover stays opaque
    // until the stage has been drawn once with the blur on; a blur that cannot be made leaves it a card.
    _blur() {
        if (this.look !== 'blur' || this._blurred || !this.widget)
            return;
        const surface = surfaceOf(this.actor, this.widget);
        if (!surface || surface.width < 1 || surface.height < 1)
            return;
        try {
            surface.add_effect_with_name(BLUR, new Shell.BlurEffect({
                mode: Shell.BlurMode.ACTOR,
                radius: BLUR_RADIUS,
                brightness: BLUR_BRIGHTNESS,
            }));
        } catch (e) {
            console.warn(`Amethystora UI: the share guard keeps a card, the blur could not be made: ${e.message}`);
            return;
        }
        this._blurred = surface;
        surface.connectObject('destroy', () => this._reblur(), this);
        global.stage.connectObject('after-paint', () => {
            global.stage.disconnectObject(this);
            // (Not whether it is mapped: in the overview the window is drawn through its clone, blur and all)
            if (this.widget && this._blurred?.get_effect(BLUR)?.enabled)
                this.widget.remove_style_class_name('amethystora-privacy-cover-opaque');
        }, this);
    }

    // The blurred picture went: opaque again until the next one is blurred
    _reblur() {
        this._unblur();
        this._blur();
    }

    _unblur() {
        global.stage.disconnectObject(this);
        this._blurred?.disconnectObject(this);
        this._blurred?.remove_effect_by_name(BLUR);
        this._blurred = null;
        this.widget?.add_style_class_name('amethystora-privacy-cover-opaque');
    }

    get blurred() {
        return !!this._blurred?.get_effect(BLUR);
    }

    get opaque() {
        return this.widget?.has_style_class_name('amethystora-privacy-cover-opaque') ?? false;
    }

    destroy() {
        this.window?.disconnectObject(this);
        this.actor?.disconnectObject(this);
        this._unblur();
        this.widget?.destroy();
        this.widget = null;
        this.window = null;
        this.actor = null;
    }
}

// The covers by window actor. The glass keeps off a covered window, which it would draw at 84 % over its pane, the cover
// with it: a card let the window show through. It is told whenever a cover comes or goes, and so are see-through
// windows (the windows module), which a cover keeps solid
class Covers extends Map {
    constructor(changed) {
        super();
        this._changed = changed;
    }

    set(actor, cover) {
        super.set(actor, cover);
        this._changed();
        return this;
    }

    delete(actor) {
        const had = super.delete(actor);
        if (had)
            this._changed();
        return had;
    }

    clear() {
        super.clear();
        this._changed();
    }
}

export class Guard {
    constructor(ext, settings) {
        this._ext = ext;
        this._settings = settings;
        // Every share and recording Mutter told of: whether it records, and whether someone else controls this computer
        this._handles = new Map();
        // The glass, and the windows module, which draws a covered window solid however see-through it was, are told
        this._covers = new Covers(() => {
            this._ext.modules?.get('glass')?.windows?.restyle();
            this._ext.modules?.get('windows')?.coversChanged?.();
        });
        // The windows' actors looked at during this share, each with its window
        this._watched = new Map();
        this._uncovered = new Set();
        this._look = 'blur';
        this._covering = false;
        this._unredirected = false;
    }

    start() {
        const controller = global.backend.get_remote_access_controller?.();
        controller?.connectObject('new-handle', (c, handle) => this.addHandle(handle), this);
        this._controller = controller;
        // The controller lists nothing: what runs already is what GNOME's own indicators hold
        const seeded = [
            ...Main.panel.statusArea.quickSettings?._remoteAccess?._handles ?? [],
            ...Main.panel.statusArea.screenSharing?._handles ?? [],
        ];
        for (const handle of seeded)
            this.addHandle(handle, true);
        this._evaluate();
    }

    stop() {
        this._controller?.disconnectObject(this);
        this._controller = null;
        for (const handle of this._handles.keys())
            handle.disconnectObject(this);
        this._handles.clear();
        this._cancellable?.cancel();
        this._cancellable = null;
        if (this._settle)
            GLib.source_remove(this._settle);
        this._settle = 0;
        this._lift();
    }

    setLook(look) {
        this._look = look;
        for (const cover of this._covers.values())
            cover.setLook(look);
    }

    addHandle(handle, quietly = false) {
        if (!handle || this._handles.has(handle))
            return;
        // Mutter's remote desktop sessions give another this computer's pointer and keyboard
        const remote = /RemoteDesktop/.test(`${handle.constructor?.name ?? ''} ${String(handle)}`);
        this._handles.set(handle, {recording: !!(handle.is_recording ?? handle.isRecording), remote});
        handle.connectObject('stopped', () => {
            handle.disconnectObject(this);
            this._handles.delete(handle);
            this._evaluate(true);
        }, this);
        if (!quietly)
            this._evaluate();
    }

    // A share that records is GNOME's recorder or another recording: not a share
    get _sharing() {
        return [...this._handles.values()].some(h => !h.recording);
    }

    get _controlled() {
        return [...this._handles.values()].some(h => h.remote);
    }

    // Covered at once; lifted only when Mutter says every stream is a window's
    _evaluate(stopped = false) {
        this._cancellable?.cancel();
        this._cancellable = null;
        if (!this._sharing) {
            this._lift();
            return;
        }
        this._cover();
        const cancellable = this._cancellable = new Gio.Cancellable();
        windowsOnly(cancellable).then(windows => {
            if (cancellable.is_cancelled() || this._cancellable !== cancellable)
                return;
            this._cancellable = null;
            if (windows)
                this._lift();
            else
                this._announce();
        }).catch(e => {
            if (!e.matches?.(Gio.IOErrorEnum, Gio.IOErrorEnum.CANCELLED))
                console.error(`Amethystora UI: the share guard could not ask what is shared: ${e.message}`);
        });
        // Mutter may still list the streams of the share that just stopped
        if (stopped && !this._settle) {
            this._settle = GLib.timeout_add(GLib.PRIORITY_DEFAULT, SETTLE_MS, () => {
                this._settle = 0;
                this._evaluate();
                return GLib.SOURCE_REMOVE;
            });
        }
    }

    // --- Covering -----------------------------------------------------------------------------------------------------

    _cover() {
        if (this._covering)
            return;
        this._covering = true;
        this._announced = null;
        global.compositor.disable_unredirect();
        this._unredirected = true;
        global.window_group.connectObject('child-added', (group, actor) => this._consider(actor), this);
        // A window's app is known once the tracker has it: when the window is made, and when the tracker changes its mind
        global.display.connectObject('window-created', (display, window) =>
            this._consider(window.get_compositor_private()), this);
        Shell.WindowTracker.get_default().connectObject('tracked-windows-changed', () => this._considerAll(), this);
        Shell.AppSystem.get_default().connectObject('installed-changed', () => this.appsChanged(), this);
        if (this._settings.get_boolean('share-guard-private'))
            this._watchFiles();
        this._considerAll();
    }

    _lift() {
        this._uncovered.clear();
        if (!this._covering)
            return;
        this._covering = false;
        global.window_group.disconnectObject(this);
        global.display.disconnectObject(this);
        Shell.WindowTracker.get_default().disconnectObject(this);
        Shell.AppSystem.get_default().disconnectObject(this);
        this._filesCancellable?.cancel();
        this._filesCancellable = null;
        this._files?.disconnectObject(this);
        this._files = null;
        this._locations = {};
        for (const [actor, window] of this._watched) {
            actor.disconnectObject(this);
            window.disconnectObject(this);
        }
        this._watched.clear();
        for (const cover of this._covers.values())
            cover.destroy();
        this._covers.clear();
        if (this._unredirected)
            global.compositor.enable_unredirect();
        this._unredirected = false;
    }

    // The list or the Private folder's switch changed: every window is looked at again
    appsChanged() {
        if (!this._covering)
            return;
        const wantFiles = this._settings.get_boolean('share-guard-private');
        if (wantFiles && !this._files && !this._filesCancellable)
            this._watchFiles();
        this._considerAll();
    }

    // One window that fails to be looked at leaves the others covered, and itself a card
    _considerAll() {
        for (const actor of global.get_window_actors())
            this._consider(actor);
    }

    // Covers the window if it is one of the chosen apps' (or a Files window in the Private folder), uncovers it if no
    // longer; watches a new window for the name it gives itself late
    _consider(actor) {
        if (!(actor instanceof Meta.WindowActor) || !this._covering)
            return;
        try {
            this._decide(actor);
        } catch (e) {
            console.error(`Amethystora UI: the share guard covers a window as a card after an error: ${e.message}`);
            if (!this._covers.has(actor) && actor.get_meta_window())
                this._covers.set(actor, new Cover(actor, null, 'card', w => this._uncover(w)));
        }
    }

    _decide(actor) {
        const window = actor.get_meta_window();
        if (!window)
            return;
        if (!this._watched.has(actor)) {
            this._watched.set(actor, window);
            actor.connectObject('destroy', () => this._forget(actor), this);
            window.connectObject(
                'notify::wm-class', () => this._consider(actor),
                'notify::gtk-application-id', () => this._consider(actor),
                this);
        }
        const app = this._appOf(window);
        const wanted = window.get_window_type() !== Meta.WindowType.DESKTOP && !this._isUncovered(window) &&
            (this._chosen(window, app) || this._private(window, app));
        const cover = this._covers.get(actor);
        if (wanted && !cover)
            this._covers.set(actor, new Cover(actor, app, this._look, w => this._uncover(w)));
        else if (!wanted && cover) {
            cover.destroy();
            this._covers.delete(actor);
        }
    }

    _forget(actor) {
        this._watched.get(actor)?.disconnectObject(this);
        this._watched.delete(actor);
        this._covers.get(actor)?.destroy();
        this._covers.delete(actor);
    }

    // The app, or, for a window the tracker knows only as itself, the installed app its class names
    _appOf(window) {
        const app = Shell.WindowTracker.get_default().get_window_app(window);
        if (!app?.is_window_backed())
            return app;
        const wmClass = window.get_wm_class?.();
        const apps = Shell.AppSystem.get_default();
        return (wmClass && (apps.lookup_startup_wmclass(wmClass) ?? apps.lookup_app(`${wmClass}.desktop`))) || app;
    }

    // A menu or a dialog of a window uncovered for this share is uncovered too
    _isUncovered(window) {
        for (let w = window; w; w = w.get_transient_for()) {
            if (this._uncovered.has(w))
                return true;
        }
        return false;
    }

    _chosen(window, app) {
        const chosen = new Set(this._settings.get_strv('share-guard-apps'));
        const wmClass = window.get_wm_class?.() ?? null;
        const ids = [app?.get_id(), window.get_sandboxed_app_id?.(), window.get_gtk_application_id?.(), wmClass,
            wmClass ? Shell.AppSystem.get_default().lookup_startup_wmclass(wmClass)?.get_id() : null]
            .filter(id => id).map(id => id.endsWith('.desktop') ? id : `${id}.desktop`);
        return ids.some(id => chosen.has(id));
    }

    // A Files window that shows a place in ~/Private, by what Files publishes of its windows
    _private(window, app) {
        if (!this._settings.get_boolean('share-guard-private') || app?.get_id() !== FILES)
            return false;
        const uris = this._locations?.[window.get_gtk_window_object_path?.() ?? ''] ?? [];
        const root = GLib.filename_to_uri(GLib.build_filenamev([GLib.get_home_dir(), 'Private']), null);
        return uris.some(uri => inside(uri, root));
    }

    _watchFiles() {
        const cancellable = this._filesCancellable = new Gio.Cancellable();
        this._locations = {};
        // Only while Files runs; asking does not start it
        Gio.DBusProxy.new_for_bus(Gio.BusType.SESSION, Gio.DBusProxyFlags.DO_NOT_AUTO_START, null,
            'org.freedesktop.FileManager1', '/org/freedesktop/FileManager1', 'org.freedesktop.FileManager1',
            cancellable, (object, result) => {
                let proxy;
                try {
                    proxy = Gio.DBusProxy.new_for_bus_finish(result);
                } catch {
                    return;
                }
                if (cancellable.is_cancelled() || !this._covering)
                    return;
                this._filesCancellable = null;
                this._files = proxy;
                proxy.connectObject('g-properties-changed', () => this._readLocations(), this);
                this._readLocations();
            });
    }

    _readLocations() {
        const value = this._files?.get_cached_property('OpenWindowsWithLocations')?.deepUnpack() ?? {};
        this._locations = typeof value === 'object' && value ? value : {};
        this._considerAll();
    }

    // Not while someone else controls this computer through the share: the hold could be theirs
    _uncover(window) {
        if (this._controlled)
            return;
        this._uncovered.add(window);
        for (const [actor, cover] of [...this._covers]) {
            if (this._isUncovered(cover.window))
                this._consider(actor);
        }
    }

    // The pill says once what the share does not show, when a screen is known to be shared
    _announce() {
        if (this._announced !== null && this._announced !== undefined)
            return;
        const names = [];
        let files = false;
        for (const cover of this._covers.values()) {
            if (cover.app?.get_id() === FILES) {
                files = true;
                continue;
            }
            // A window the tracker knows only as itself has no name worth saying (its class or its title)
            const name = cover.app?.is_window_backed() ? _('another app') : plain(cover.app?.get_name() ?? '', 40);
            if (name && !names.includes(name))
                names.push(name);
        }
        if (files)
            names.push(_('the Private folder'));
        this._announced = '';
        if (names.length === 0)
            return;
        const list = new Intl.ListFormat(undefined, {style: 'long', type: 'conjunction'}).format(names);
        this._announced = _('Covered while you share: %s').format(list);
        this._ext.modules?.get('pill')?.notice?.(this._announced, 'view-conceal-symbolic', NOTICE_SECONDS);
    }

    // --- For the VM suite ---------------------------------------------------------------------------------------------

    state() {
        return {
            sharing: this._sharing,
            covering: this._covering,
            controlled: this._controlled,
            unredirectDisabled: this._unredirected,
            look: this._look,
            announced: this._announced ?? null,
            covers: [...this._covers.values()].map(c => ({
                app: c.app?.get_id() ?? null,
                title: c.window?.get_title() ?? '',
                look: c.look,
                blur: c.blurred,
                opaque: c.opaque,
                onTop: c.widget?.get_parent()?.get_last_child() === c.widget,
                width: c.widget?.width ?? 0,
                height: c.widget?.height ?? 0,
            })),
            uncovered: this._uncovered.size,
            files: !!this._files,
            handles: this._handles.size,
            watched: this._watched.size,
        };
    }

    covers(window) {
        return this._covers.has(window.get_compositor_private());
    }

    // The cover of the window whose title is given, for the suite to hold a click on
    coverOf(title) {
        return [...this._covers.values()].find(c => c.window?.get_title() === title)?.widget ?? null;
    }
}
