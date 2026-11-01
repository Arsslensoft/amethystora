// Glance: Space on a file the search found shows it in GNOME's previewer (the preinstalled org.gnome.NautilusPreviewer)
// without opening it, over the search box, which steps aside while the preview is up and keeps its text, results and
// selection. The arrow keys in the previewer go to the previous or next file of the results; Space or Escape closes the
// preview and the search is there again, with the keyboard on the file last shown. Only a file:// address that Files'
// own search gave is ever handed to the previewer; nothing is renamed or moved to the trash from here.
//
// The previewer is asked over the session bus, the first time Glance is used in a search, and the connection goes
// when the search box gives its search back.

import Clutter from 'gi://Clutter';
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import Shell from 'gi://Shell';
import St from 'gi://St';

import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import * as Search from 'resource:///org/gnome/shell/ui/search.js';
import {gettext as _} from 'resource:///org/gnome/shell/extensions/extension.js';

const BUS_NAME = 'org.gnome.NautilusPreviewer';
const OBJECT = '/org/gnome/NautilusPreviewer';
const INTERFACE = 'org.gnome.NautilusPreviewer2';
const PREVIEWER_ID = 'org.gnome.NautilusPreviewer';
const PREVIEWER = `${PREVIEWER_ID}.desktop`;
const FILES = 'org.gnome.Nautilus.desktop';
// Gtk.DirectionType, as the previewer sends its arrow keys
const BACK = new Set([2, 4]);
const ON = new Set([3, 5]);
// Pixels between the previewer's window and the hint under it
const GAP = 12;

// The file:// address of a result of Files' own search, or null for anything else
export function fileOf(result) {
    if (!(result instanceof Search.SearchResult))
        return null;
    const provider = result.provider;
    if (!provider?.isRemoteProvider || provider.appInfo?.get_id?.() !== FILES)
        return null;
    const id = result.metaInfo?.id;
    if (typeof id !== 'string')
        return null;
    try {
        const uri = GLib.Uri.parse(id, GLib.UriFlags.NONE);
        if (uri.get_scheme() !== 'file' || Gio.File.new_for_uri(id).get_path() === null)
            return null;
    } catch {
        return null;
    }
    return id;
}

export class Glance {
    // host: the search box, which knows its file results, steps aside and comes back
    constructor(host) {
        this._host = host;
        this._proxy = null;
        this._showing = null;
        this._window = null;
        this._pill = null;
        this._watched = new Set();
        this._propertiesId = 0;
    }

    get active() {
        return this._showing !== null;
    }

    // Space on a file result
    show(result) {
        const uri = fileOf(result);
        if (!uri)
            return false;
        this._showing = result;
        this._host.glanceStarted(result);
        this._ensureProxy(() => this._ask(uri));
        global.display.connectObject(
            'window-created', (display, window) => this._found(window, true),
            'notify::focus-window', () => this._focusMoved(),
            this);
        for (const actor of global.get_window_actors())
            this._found(actor.meta_window, false);
        return true;
    }

    _ensureProxy(then) {
        if (this._proxy) {
            then();
            return;
        }
        if (this._making) {
            this._then = then;
            return;
        }
        this._making = new Gio.Cancellable();
        this._then = then;
        Gio.DBusProxy.new_for_bus(Gio.BusType.SESSION, Gio.DBusProxyFlags.DO_NOT_LOAD_PROPERTIES,
            null, BUS_NAME, OBJECT, INTERFACE, this._making,
            (source, result) => {
                let proxy = null;
                try {
                    proxy = Gio.DBusProxy.new_for_bus_finish(result);
                } catch (e) {
                    if (!e.matches?.(Gio.IOErrorEnum, Gio.IOErrorEnum.CANCELLED))
                        console.warn(`Amethystora UI: the previewer could not be reached: ${e.message}`);
                }
                this._making = null;
                if (!proxy) {
                    this.end();
                    return;
                }
                this._proxy = proxy;
                proxy.connectObject('g-signal', (p, sender, signal, parameters) => this._signal(signal, parameters),
                    this);
                // Its properties are not loaded (that would start it): its Visible going false is heard here
                this._propertiesId = Gio.DBus.session.signal_subscribe(BUS_NAME, 'org.freedesktop.DBus.Properties',
                    'PropertiesChanged', OBJECT, INTERFACE, Gio.DBusSignalFlags.NONE, (c, s, o, i, n, parameters) => {
                        const [, changed] = parameters.deep_unpack();
                        if (changed.Visible && !changed.Visible.unpack())
                            this.end();
                    });
                const next = this._then;
                this._then = null;
                next?.();
            });
    }

    _ask(uri) {
        if (!this._proxy || !this._showing)
            return;
        let token = '';
        try {
            const context = global.create_app_launch_context(global.get_current_time(), -1);
            token = context.get_startup_notify_id(Gio.DesktopAppInfo.new(PREVIEWER), []) ?? '';
        } catch {}
        this._proxy.call('ShowFile', new GLib.Variant('(ssbs)', [uri, '', false, token]),
            Gio.DBusCallFlags.NONE, -1, null, (proxy, result) => {
                try {
                    proxy.call_finish(result);
                } catch (e) {
                    console.warn(`Amethystora UI: the previewer did not show the file: ${e.message}`);
                    this.end();
                }
            });
    }

    // SelectionEvent: q in GNOME 50's previewer, u in 51's; the first value either way
    _signal(signal, parameters) {
        // The proxy is untyped: a signal without the value it should carry is not the previewer's
        if (!this._showing || !parameters || parameters.n_children() < 1)
            return;
        if (signal === 'SelectionEvent') {
            const direction = Number(parameters.get_child_value(0).unpack());
            const step = BACK.has(direction) ? -1 : ON.has(direction) ? 1 : 0;
            if (!step)
                return;
            const files = this._host.fileResults();
            const next = files[files.indexOf(this._showing) + step];
            if (!next)
                return;
            this._showing = next;
            this._host.glanceSelected(next);
            this._ask(fileOf(next));
        } else if (signal === 'NavigateTo') {
            const uri = parameters.get_child_value(0).unpack();
            this.end();
            this._host.closeForFiles?.();
            if (typeof uri === 'string' && uri.startsWith('file://')) {
                Gio.DBus.session.call('org.freedesktop.FileManager1', '/org/freedesktop/FileManager1',
                    'org.freedesktop.FileManager1', 'ShowItems', new GLib.Variant('(ass)', [[uri], '']),
                    null, Gio.DBusCallFlags.NONE, -1, null, null);
            }
        } else if (signal === 'Rename' || signal === 'Trash') {
            this.end();
        }
    }

    // By its sandbox first: Flathub's previewer has no launcher, so GNOME finds no app for its window, and Glance took
    // the previewer's own window for another one and closed itself
    _isPreviewer(window) {
        return window.get_sandboxed_app_id() === PREVIEWER_ID ||
            Shell.WindowTracker.get_default().get_window_app(window)?.get_id() === PREVIEWER;
    }

    // The previewer's window: the hint goes under it and follows it
    _found(window, watchShown) {
        if (!window || this._window || !this._showing)
            return;
        // The app is known once the window is shown
        const check = () => {
            if (this._window || !this._isPreviewer(window))
                return;
            this._window = window;
            window.connectObject(
                'position-changed', () => this._placePill(),
                'size-changed', () => this._placePill(),
                'unmanaged', () => this.end(),
                this);
            this._makePill();
            this._placePill();
        };
        check();
        if (!this._window && watchShown) {
            window.connectObject('shown', () => check(), this);
            this._watched.add(window);
        }
    }

    // The focus went to a window other than the previewer's: the preview and the search both close
    _focusMoved() {
        const focus = global.display.focus_window;
        if (!this._showing || !focus || this._isPreviewer(focus))
            return;
        this._proxy?.call('Close', null, Gio.DBusCallFlags.NONE, -1, null, null);
        this.end();
        this._host.closeForFocus?.();
    }

    _makePill() {
        if (this._pill)
            return;
        this._pill = new St.BoxLayout({style_class: 'amethystora-search-glance-hint', reactive: false});
        const chip = text => new St.Label({style_class: 'amethystora-search-key', text, y_align: Clutter.ActorAlign.CENTER});
        const words = text => new St.Label({style_class: 'amethystora-search-glance-words', text,
            y_align: Clutter.ActorAlign.CENTER});
        this._pill.add_child(chip('↑'));
        this._pill.add_child(chip('↓'));
        this._pill.add_child(words(_('next result')));
        this._pill.add_child(new St.Widget({style_class: 'amethystora-search-glance-gap'}));
        this._pill.add_child(chip(_('Space')));
        this._pill.add_child(words(_('closes, the search stays')));
        Main.layoutManager.addChrome(this._pill);
        this._host.glassChanged();
    }

    _placePill() {
        if (!this._pill || !this._window)
            return;
        const frame = this._window.get_frame_rect();
        const area = Main.layoutManager.getWorkAreaForMonitor(this._window.get_monitor());
        const [, width] = this._pill.get_preferred_width(-1);
        const [, height] = this._pill.get_preferred_height(width);
        const scale = St.ThemeContext.get_for_stage(global.stage).scale_factor;
        let y = frame.y + frame.height + GAP * scale;
        if (y + height > area.y + area.height)
            y = frame.y - GAP * scale - height;
        const x = Math.round(frame.x + (frame.width - width) / 2);
        this._pill.set_position(Math.max(area.x, Math.min(x, area.x + area.width - width)), Math.max(area.y, Math.round(y)));
    }

    get pill() {
        return this._pill;
    }

    // The preview closed (its own Space or Escape, a window that went, an error): the search comes back
    end() {
        if (!this._showing && !this._pill)
            return;
        const last = this._showing;
        this._showing = null;
        global.display.disconnectObject(this);
        this._window?.disconnectObject(this);
        for (const window of this._watched)
            window.disconnectObject(this);
        this._watched.clear();
        this._window = null;
        if (this._pill) {
            this._pill.destroy();
            this._pill = null;
            this._host.glassChanged();
        }
        this._host.glanceEnded(last);
    }

    // The search gives its search back: the connection to the previewer goes too
    destroy() {
        this._making?.cancel();
        this._making = null;
        this._then = null;
        if (this._showing)
            this._proxy?.call('Close', null, Gio.DBusCallFlags.NONE, -1, null, null);
        this.end();
        this._proxy?.disconnectObject(this);
        this._proxy = null;
        if (this._propertiesId)
            Gio.DBus.session.signal_unsubscribe(this._propertiesId);
        this._propertiesId = 0;
    }
}
