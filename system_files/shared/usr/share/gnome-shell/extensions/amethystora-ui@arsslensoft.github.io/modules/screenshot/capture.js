// Turning what the screenshot UI froze into a picture, and putting the picture where it goes: the clipboard, the
// screenshots folder GNOME uses (under its own translated name, so that every screenshot lands in one place), the
// recent files, or a place the person picks through the file chooser portal.

import Cairo from 'cairo';
import Cogl from 'gi://Cogl';
import GdkPixbuf from 'gi://GdkPixbuf';
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import Shell from 'gi://Shell';
import St from 'gi://St';

import Gettext from 'gettext';

import {gettext as _} from 'resource:///org/gnome/shell/extensions/extension.js';

import {paintMarks} from './marks.js';

// Idempotent: GNOME's screenshot UI makes the same call when it loads
Gio._promisify(Shell.Screenshot, 'composite_to_stream');
Gio._promisify(Gio.File.prototype, 'load_contents_async', 'load_contents_finish');

export const GRADIA = 'be.alexandervanhee.gradia';

// Gradia counts as there when the shell knows its launcher and the launcher is one Flatpak exported
export function gradiaInstalled() {
    const app = Shell.AppSystem.get_default().lookup_app(`${GRADIA}.desktop`);
    return Boolean(app?.get_app_info()?.get_string('X-Flatpak'));
}

export function playShutter() {
    global.display.get_sound_player().play_from_theme('screen-capture', _('A screenshot was taken'), null);
}

// A texture as a picture: the shell's own compositor of screenshots, which also draws the pointer in
async function compose(texture, [x, y, w, h], scale, cursor) {
    const stream = Gio.MemoryOutputStream.new_resizable();
    const pixbuf = await Shell.Screenshot.composite_to_stream(texture, x, y, w, h, scale,
        cursor?.texture ?? null, cursor?.x ?? 0, cursor?.y ?? 0, cursor?.scale ?? 1, stream);
    stream.close(null);
    return {bytes: stream.steal_as_bytes(), width: pixbuf.get_width(), height: pixbuf.get_height(), pixbuf};
}

// A file of our own for a moment, in the runtime folder, which is memory and the person's alone
function scratchFile() {
    return Gio.File.new_for_path(GLib.build_filenamev([GLib.get_user_runtime_dir(),
        `amethystora-screenshot-${GLib.uuid_string_random()}.png`]));
}

// Picture data to and from pixbufs: GdkPixbuf's own worker threads do the decoding and the encoding, and the main
// loop only waits for the answer
function decodePng(bytes) {
    return new Promise((resolve, reject) => {
        const stream = Gio.MemoryInputStream.new_from_bytes(bytes);
        GdkPixbuf.Pixbuf.new_from_stream_async(stream, null, (source, result) => {
            try {
                resolve(GdkPixbuf.Pixbuf.new_from_stream_finish(result));
            } catch (e) {
                reject(e);
            }
        });
    });
}

// Bytes into a file, written by Gio's worker threads: a folder on a network share, or on one that has stopped
// answering, holds up the write and not the compositor
function writeBytes(file, bytes) {
    return new Promise((resolve, reject) => {
        file.replace_contents_bytes_async(bytes, null, false, Gio.FileCreateFlags.REPLACE_DESTINATION, null,
            (f, result) => {
                try {
                    f.replace_contents_finish(result);
                    resolve();
                } catch (e) {
                    reject(e);
                }
            });
    });
}

function encodePng(pixbuf) {
    return new Promise((resolve, reject) => {
        const stream = Gio.MemoryOutputStream.new_resizable();
        pixbuf.save_to_streamv_async(stream, 'png', [], [], null, (source, result) => {
            try {
                GdkPixbuf.Pixbuf.save_to_stream_finish(result);
                stream.close(null);
                resolve(stream.steal_as_bytes());
            } catch (e) {
                reject(e);
            }
        });
    });
}

// The marks painted over a picture whose top-left is the stage point (x, y), at `scale` pixels a logical pixel. They
// are painted on a clear surface of the picture's size, which is then laid over the picture's pixbuf, so that the
// picture itself is never unpacked and packed again here: its encoding is GdkPixbuf's, in a worker thread.
async function paintOnto(picture, marks, x, y, scale) {
    const canvas = picture.pixbuf ?? await decodePng(picture.bytes);
    const overlay = new Cairo.ImageSurface(Cairo.Format.ARGB32, canvas.get_width(), canvas.get_height());
    const cr = new Cairo.Context(overlay);
    paintMarks(cr, marks, x, y, scale);
    cr.$dispose();

    // The part that stays on the main loop: cairo, in GJS, paints only where it is called and hands its pixels over
    // only as a PNG written under a file name, so the marks are written out (the surface is clear but for them, so
    // the PNG is small and quick, and the folder is memory) and read back by a worker thread.
    const file = scratchFile();
    let layer;
    try {
        overlay.writeToPNG(file.get_path());
        overlay.finish();
        const [contents] = await file.load_contents_async(null);
        layer = await decodePng(GLib.Bytes.new(contents));
    } finally {
        file.delete_async(GLib.PRIORITY_DEFAULT, null, (f, result) => {
            try {
                f.delete_finish(result);
            } catch {}
        });
    }
    layer.composite(canvas, 0, 0, canvas.get_width(), canvas.get_height(), 0, 0, 1, 1,
        GdkPixbuf.InterpType.NEAREST, 255);
    return {...picture, bytes: await encodePng(canvas), pixbuf: canvas};
}

// The area or the screen chosen in the UI, with the pointer when its toggle is on, and the marks over it
export async function captureFrame(ui, marks) {
    const content = ui._stageScreenshot.get_content();
    if (!content)
        return null;
    const scale = ui._scale;
    const [x, y] = ui._getSelectedGeometry(false);
    const geometry = ui._getSelectedGeometry(true);
    const cursorTexture = ui._cursor.visible ? ui._cursor.content?.get_texture() : null;
    const cursor = cursorTexture
        ? {texture: cursorTexture, x: ui._cursor.x * scale, y: ui._cursor.y * scale, scale: ui._cursorScale}
        : null;
    const picture = await compose(content.get_texture(), geometry, scale, cursor);
    return marks.length > 0 ? paintOnto(picture, marks, x, y, scale) : picture;
}

// The chosen window drawn over the windows it belongs to. A window that belongs to none comes out as GNOME's own
// window capture would; a parent is painted now, the chosen window as the UI froze it.
export async function captureWindow(ui, chosen, metaWindow) {
    const content = chosen.windowContent;
    if (!content)
        return null;
    const scale = chosen.bufferScale;
    const cursorTexture = ui._cursor.visible ? chosen.getCursorTexture()?.get_texture() : null;
    const point = chosen.cursorPoint;

    const parents = [];
    for (let parent = metaWindow?.get_transient_for(); parent; parent = parent.get_transient_for()) {
        if (parents.includes(parent))
            break;
        parents.unshift(parent);
    }
    const actors = parents.map(w => w.get_compositor_private()).filter(a => a);
    if (actors.length === 0) {
        const cursor = cursorTexture
            ? {texture: cursorTexture, x: point.x * scale, y: point.y * scale, scale: ui._cursorScale}
            : null;
        return compose(content.get_texture(), [0, 0, -1, -1], scale, cursor);
    }

    // From the top-level window down to the chosen one, each at its place on the screen, whole buffers
    const layers = actors.map(actor => ({
        rect: actor.metaWindow.get_buffer_rect(),
        texture: actor.paint_to_content(null)?.get_texture(),
        scale: actor.get_resource_scale(),
    })).filter(l => l.texture);
    const own = metaWindow.get_buffer_rect();
    layers.push({rect: own, texture: content.get_texture(), scale});

    const left = Math.min(...layers.map(l => l.rect.x));
    const top = Math.min(...layers.map(l => l.rect.y));
    const right = Math.max(...layers.map(l => l.rect.x + l.rect.width));
    const bottom = Math.max(...layers.map(l => l.rect.y + l.rect.height));
    const width = Math.ceil((right - left) * scale);
    const height = Math.ceil((bottom - top) * scale);
    const canvas = GdkPixbuf.Pixbuf.new(GdkPixbuf.Colorspace.RGB, true, 8, width, height);
    canvas.fill(0);

    const place = (pixbuf, x, y, factor) => {
        const dx = Math.round((x - left) * scale);
        const dy = Math.round((y - top) * scale);
        const x1 = Math.max(0, dx);
        const y1 = Math.max(0, dy);
        const x2 = Math.min(width, dx + Math.round(pixbuf.get_width() * factor));
        const y2 = Math.min(height, dy + Math.round(pixbuf.get_height() * factor));
        if (x2 > x1 && y2 > y1) {
            pixbuf.composite(canvas, x1, y1, x2 - x1, y2 - y1, dx, dy, factor, factor,
                GdkPixbuf.InterpType.BILINEAR, 255);
        }
    };
    for (const layer of layers) {
        // eslint-disable-next-line no-await-in-loop
        const {pixbuf} = await compose(layer.texture, [0, 0, -1, -1], 1, null);
        place(pixbuf, layer.rect.x, layer.rect.y, scale / layer.scale);
    }
    if (cursorTexture) {
        const {pixbuf} = await compose(cursorTexture, [0, 0, -1, -1], 1, null);
        place(pixbuf, own.x + point.x, own.y + point.y, ui._cursorScale * scale);
    }

    return {bytes: await encodePng(canvas), width, height, pixbuf: canvas};
}

export function copyToClipboard(picture) {
    St.Clipboard.get_default().set_content(St.ClipboardType.CLIPBOARD, 'image/png', picture.bytes);
}

// One for the module's life, disposed by its disable() rather than by the collector (dock.js says why)
let lockdown = null;

function savingAllowed() {
    lockdown ??= new Gio.Settings({schema_id: 'org.gnome.desktop.lockdown'});
    return !lockdown.get_boolean('disable-save-to-disk');
}

export function disposeSettings() {
    lockdown?.run_dispose();
    lockdown = null;
}

// The name GNOME gives a screenshot taken now, in the session's language as GNOME words it
export function suggestedName() {
    const stamp = GLib.DateTime.new_now_local().format('%Y-%m-%d %H-%M-%S');
    return Gettext.dgettext('gnome-shell', 'Screenshot From %s').replace('%s', stamp);
}

function addToRecent(file) {
    const path = GLib.build_filenamev([GLib.get_user_data_dir(), 'recently-used.xbel']);
    const bookmarks = new GLib.BookmarkFile();
    try {
        bookmarks.load_from_file(path);
    } catch (e) {
        if (!e.matches(GLib.FileError, GLib.FileError.NOENT)) {
            console.warn(`Amethystora UI: the recent files cannot be read: ${e.message}`);
            return;
        }
    }
    try {
        bookmarks.add_application(file.get_uri(), GLib.get_prgname(), 'gio open %u');
        bookmarks.to_file(path);
    } catch (e) {
        console.warn(`Amethystora UI: the screenshot was not added to the recent files: ${e.message}`);
    }
}

// The folder GNOME keeps screenshots in: Screenshots, in the session's language, under the pictures folder
function screenshotsFolder() {
    const pictures = GLib.get_user_special_dir(GLib.UserDirectory.DIRECTORY_PICTURES) || GLib.get_home_dir();
    return Gio.File.new_for_path(GLib.build_filenamev([pictures, Gettext.dgettext('gnome-shell', 'Screenshots')]));
}

// Into the screenshots folder, under a name nothing has yet; null when saving to disk is locked down
export function saveToFolder(picture) {
    if (!savingAllowed())
        return null;
    const folder = screenshotsFolder();
    try {
        folder.make_directory_with_parents(null);
    } catch (e) {
        if (!e.matches(Gio.IOErrorEnum, Gio.IOErrorEnum.EXISTS))
            throw e;
    }
    const name = suggestedName();
    for (let n = 0; ; n++) {
        const file = folder.get_child(n === 0 ? `${name}.png` : `${name}-${n}.png`);
        try {
            const stream = file.create(Gio.FileCreateFlags.NONE, null);
            stream.write_bytes(picture.bytes, null);
            stream.close(null);
            addToRecent(file);
            return file;
        } catch (e) {
            if (!e.matches(Gio.IOErrorEnum, Gio.IOErrorEnum.EXISTS))
                throw e;
        }
    }
}

// The chooser opens where screenshots are kept, or the pictures folder before the first one
function startFolder() {
    const folder = screenshotsFolder();
    return folder.query_exists(null) ? folder.get_path() : folder.get_parent().get_path();
}

// Where the person wants it, asked through the file chooser portal; resolves to the file written, or null when
// they cancel. `pending` keeps the question's signal, so that turning the module off stops waiting for it.
export function saveAs(picture, pending) {
    if (!savingAllowed())
        return Promise.resolve(null);
    const bus = Gio.DBus.session;
    const token = `amethystora_${GLib.uuid_string_random().replaceAll('-', '')}`;
    const sender = bus.get_unique_name().slice(1).replaceAll('.', '_');
    const request = `/org/freedesktop/portal/desktop/request/${sender}/${token}`;

    return new Promise(resolve => {
        // Listened for before asking, so that a quick answer is not missed
        const id = bus.signal_subscribe('org.freedesktop.portal.Desktop', 'org.freedesktop.portal.Request',
            'Response', request, null, Gio.DBusSignalFlags.NONE, (c, s, p, i, n, parameters) => {
                bus.signal_unsubscribe(id);
                pending.delete(id);
                const [code, results] = parameters.recursiveUnpack();
                const uri = code === 0 ? results.uris?.[0] : null;
                if (!uri) {
                    resolve(null);
                    return;
                }
                // Wherever the person chose, which may be slow to answer
                const file = Gio.File.new_for_uri(uri);
                writeBytes(file, picture.bytes).then(() => {
                    addToRecent(file);
                    resolve(file);
                }).catch(e => {
                    console.warn(`Amethystora UI: the screenshot was not saved: ${e.message}`);
                    resolve(null);
                });
            });
        pending.add(id);
        const png = [_('PNG image'), [[0, '*.png'], [1, 'image/png']]];
        const options = {
            handle_token: new GLib.Variant('s', token),
            current_name: new GLib.Variant('s', `${suggestedName()}.png`),
            current_folder: new GLib.Variant('ay', new TextEncoder().encode(`${startFolder()}\0`)),
            filters: new GLib.Variant('a(sa(us))', [png]),
            current_filter: new GLib.Variant('(sa(us))', png),
        };
        bus.call('org.freedesktop.portal.Desktop', '/org/freedesktop/portal/desktop',
            'org.freedesktop.portal.FileChooser', 'SaveFile',
            new GLib.Variant('(ssa{sv})', ['', _('Save the screenshot as'), options]),
            null, Gio.DBusCallFlags.NONE, -1, null, (connection, result) => {
                try {
                    connection.call_finish(result);
                } catch (e) {
                    console.warn(`Amethystora UI: the file chooser did not open: ${e.message}`);
                    bus.signal_unsubscribe(id);
                    pending.delete(id);
                    resolve(null);
                }
            });
    });
}

// The picture as the preview card shows it
export function contentOf(picture) {
    let pixbuf = picture.pixbuf;
    if (!pixbuf) {
        const stream = Gio.MemoryInputStream.new_from_bytes(picture.bytes);
        pixbuf = GdkPixbuf.Pixbuf.new_from_stream(stream, null);
        stream.close(null);
    }
    if (!pixbuf.get_has_alpha())
        pixbuf = pixbuf.add_alpha(false, 0, 0, 0);
    const context = global.stage.context.get_backend().get_cogl_context();
    const content = St.ImageContent.new_with_preferred_size(pixbuf.get_width(), pixbuf.get_height());
    content.set_bytes(context, pixbuf.read_pixel_bytes(), Cogl.PixelFormat.RGBA_8888,
        pixbuf.get_width(), pixbuf.get_height(), pixbuf.get_rowstride());
    return content;
}
