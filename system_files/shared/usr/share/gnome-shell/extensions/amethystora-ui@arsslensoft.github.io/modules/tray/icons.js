// Where a tray item's picture comes from. An item names an icon, gives a file, or sends the pixels themselves; a
// name that is found always wins over pixels. Names are looked up in the item's own folder when it gives one the
// icon theme does not already search, else in the icon theme. A file the user's programs can write is read afresh
// each time, so that a program rewriting the same file shows the new picture, and a wide one (a graph) keeps its
// width.
//
// All of it comes from programs that are not trusted to be small or quick, and is drawn on the shell's own thread, so
// every picture has a ceiling: a pixmap is skipped beyond PIXMAP_SIDE pixels, a file beyond FILE_BYTES and FILE_SIDE,
// a file is unpacked by a worker thread, and an item's own folder is looked at off the main thread before it is
// searched (see _folderTheme).

import Cogl from 'gi://Cogl';
import GdkPixbuf from 'gi://GdkPixbuf';
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import St from 'gi://St';

Gio._promisify(Gio.File.prototype, 'query_info_async');
Gio._promisify(Gio.File.prototype, 'query_filesystem_info_async');
Gio._promisify(Gio.File.prototype, 'enumerate_children_async');
Gio._promisify(Gio.FileEnumerator.prototype, 'next_files_async');

// A picture this much wider than tall is shown at its own width, up to WIDE_MOST times its height
const WIDE = 1.5;
const WIDE_MOST = 16;
const IMAGE_SUFFIX = /\.(png|svg)$/i;
// The most a pixmap may be on a side: a tray icon is drawn between 16 and 64 pixels
const PIXMAP_SIDE = 512;
// How many sizes of one pixmap property are looked at
const PIXMAP_SIZES = 32;
// An icon file's most on disk, and the most it may say it is on a side
const FILE_BYTES = 1024 * 1024;
const FILE_SIDE = 1024;
// An item's own icon folder is searched only when it holds this many entries or fewer, counting what is in its
// subfolders down to this many levels, which is where an icon theme keeps its pictures (size, then context)
const FOLDER_ENTRIES = 2000;
const FOLDER_DEPTH = 3;
// How many folders keep their icon theme; the one asked for longest ago goes first
const FOLDERS_KEPT = 16;

function pixmapFits(width, height) {
    return width > 0 && height > 0 && width <= PIXMAP_SIDE && height <= PIXMAP_SIDE;
}

/**
 * Whether a pixmap property has a size the tray would draw; one that has none is not worth keeping in memory.
 *
 * @param {GLib.Variant} pixmaps - the property's value
 * @returns {boolean}
 */
export function hasUsablePixmap(pixmaps) {
    for (let i = 0; i < Math.min(pixmaps.n_children(), PIXMAP_SIZES); i++) {
        const child = pixmaps.get_child_value(i);
        if (pixmapFits(child.get_child_value(0).get_int32(), child.get_child_value(1).get_int32()))
            return true;
    }
    return false;
}

/**
 * Pixels as the shell draws them, from a pixmap property (a(iiay): width, height, ARGB32 in network byte order,
 * which is Cogl's ARGB_8888 byte for byte). Of the sizes given, the smallest at least as big as wanted, else the
 * largest; one over PIXMAP_SIDE on a side is never used, since the shell would have to keep all of it as a texture.
 *
 * @param {GLib.Variant} pixmaps - the property's value
 * @param {number} wanted - the size in device pixels
 * @returns {St.ImageContent|null}
 */
export function pixmapContent(pixmaps, wanted) {
    let best = null;
    for (let i = 0; i < Math.min(pixmaps.n_children(), PIXMAP_SIZES); i++) {
        const child = pixmaps.get_child_value(i);
        const width = child.get_child_value(0).get_int32();
        const height = child.get_child_value(1).get_int32();
        if (!pixmapFits(width, height))
            continue;
        const area = width * height;
        const bigEnough = width >= wanted && height >= wanted;
        if (!best ||
            (bigEnough && (!best.bigEnough || area < best.area)) ||
            (!bigEnough && !best.bigEnough && area > best.area))
            best = {child, width, height, area, bigEnough};
    }
    if (!best)
        return null;

    const bytes = best.child.get_child_value(2).get_data_as_bytes();
    if (bytes.get_size() < best.width * best.height * 4)
        return null;
    return imageContent(bytes, Cogl.PixelFormat.ARGB_8888, best.width, best.height, best.width * 4);
}

function imageContent(bytes, format, width, height, rowstride) {
    const content = St.ImageContent.new_with_preferred_size(width, height);
    try {
        const cogl = global.stage.context.get_backend().get_cogl_context();
        content.set_bytes(cogl, bytes, format, width, height, rowstride);
    } catch {
        return null;
    }
    return content;
}

// What a PNG says its size is, from its first chunk (always IHDR), or null for data that is not a PNG. This is what
// keeps a small file from unpacking to gigabytes. GdkPixbuf here hands pictures to glycin, which decodes them whole
// in a sandbox before the loader's size-prepared signal says anything, so the loader cannot be stopped from that
// signal (setting a size of nothing there changes nothing), and the size has to be read before the loader sees the
// data.
function pngSize(bytes) {
    if (bytes.get_size() < 24)
        return null;
    const head = GLib.Bytes.new_from_bytes(bytes, 0, 24).toArray();
    const signature = [0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a];
    const ihdr = [0x49, 0x48, 0x44, 0x52];
    if (signature.some((byte, i) => head[i] !== byte) || ihdr.some((byte, i) => head[12 + i] !== byte))
        return null;
    const view = new DataView(head.buffer, head.byteOffset, head.length);
    return {width: view.getUint32(16), height: view.getUint32(20)};
}

// Whether data looks like an SVG, plain (a byte order mark is dropped by the decoder) or gzipped: a vector picture is
// drawn at the size asked, whatever it says
function looksLikeSvg(bytes) {
    const head = GLib.Bytes.new_from_bytes(bytes, 0, Math.min(bytes.get_size(), 256)).toArray();
    if (head[0] === 0x1f && head[1] === 0x8b)
        return true;
    return new TextDecoder().decode(head).trimStart().startsWith('<');
}

/**
 * A PNG as a picture of `size` device pixels at most (a smaller one stays as it is), for what a menu shows. It is
 * unpacked on the calling thread, which is acceptable only because of its limits: the PNG must say it is no more
 * than `maxSide` on a side, and the caller bounds its bytes.
 *
 * @param {GLib.Bytes} bytes - the PNG
 * @param {number} size - the side wanted, in device pixels
 * @param {number} maxSide - the most the PNG may say it is on a side
 * @returns {St.ImageContent|null}
 */
export function pngContent(bytes, size, maxSide) {
    const natural = pngSize(bytes);
    if (!natural || natural.width < 1 || natural.height < 1 || natural.width > maxSide || natural.height > maxSide)
        return null;
    const loader = new GdkPixbuf.PixbufLoader();
    // Scaled as it loads, to fit a square of the size wanted
    loader.connect('size-prepared', (l, width, height) => {
        const scale = Math.min(size / Math.max(width, height), 1);
        l.set_size(Math.max(1, Math.round(width * scale)), Math.max(1, Math.round(height * scale)));
    });
    let pixbuf;
    try {
        loader.write_bytes(bytes);
        loader.close();
        pixbuf = loader.get_pixbuf();
    } catch {
        try {
            loader.close();
        } catch {}
        return null;
    }
    return pixbuf ? fromPixbuf(pixbuf)?.content ?? null : null;
}

// A pixbuf as a texture the shell can draw, with its size; wide when it is much wider than tall
function fromPixbuf(pixbuf) {
    const format = pixbuf.get_has_alpha() ? Cogl.PixelFormat.RGBA_8888 : Cogl.PixelFormat.RGB_888;
    const width = pixbuf.get_width();
    const height = pixbuf.get_height();
    const content = imageContent(pixbuf.read_pixel_bytes(), format, width, height, pixbuf.get_rowstride());
    return content ? {content, width, height, wide: width >= WIDE * height} : null;
}

// The picture scaled by GdkPixbuf in a worker thread, so that the main thread never waits for it: to the height
// wanted, and a wide one (a graph) at its own width, up to WIDE_MOST times as wide as tall
function decodeScaled(bytes, wanted) {
    return new Promise((resolve, reject) => {
        const stream = Gio.MemoryInputStream.new_from_bytes(bytes);
        GdkPixbuf.Pixbuf.new_from_stream_at_scale_async(stream, wanted * WIDE_MOST, wanted, true, null,
            (source, result) => {
                try {
                    resolve(GdkPixbuf.Pixbuf.new_from_stream_finish(result));
                } catch (e) {
                    reject(e);
                }
            });
    });
}

function readFile(file) {
    return new Promise((resolve, reject) => {
        file.load_contents_async(null, (f, result) => {
            try {
                resolve(new GLib.Bytes(f.load_contents_finish(result)[1]));
            } catch (e) {
                reject(e);
            }
        });
    });
}

// How many entries are left of `left` once this folder, and what is in it down to `depth` levels, is counted; below
// nought when it holds more. Read by the worker threads of Gio, a few at a time.
async function entriesLeft(folder, left, depth, cancellable) {
    const children = await folder.enumerate_children_async('standard::name,standard::type',
        Gio.FileQueryInfoFlags.NOFOLLOW_SYMLINKS, GLib.PRIORITY_DEFAULT, cancellable);
    try {
        for (;;) {
            // eslint-disable-next-line no-await-in-loop
            const infos = await children.next_files_async(64, GLib.PRIORITY_DEFAULT, cancellable);
            if (infos.length === 0)
                break;
            for (const info of infos) {
                left--;
                if (left < 0)
                    return -1;
                if (depth > 1 && info.get_file_type() === Gio.FileType.DIRECTORY) {
                    // eslint-disable-next-line no-await-in-loop
                    left = await entriesLeft(folder.get_child(info.get_name()), left, depth - 1, cancellable);
                    if (left < 0)
                        return -1;
                }
            }
        }
    } finally {
        children.close(null);
    }
    return left;
}

// The places the account's own programs write to: their files may change under the same name
function userPlaces() {
    const places = [
        GLib.get_home_dir(), GLib.get_tmp_dir(), '/var/tmp', GLib.get_user_runtime_dir(),
        GLib.get_user_cache_dir(), GLib.get_user_data_dir(), GLib.get_user_config_dir(),
    ];
    for (const key of Object.keys(GLib.UserDirectory)) {
        const dir = GLib.UserDirectory[key];
        if (typeof dir === 'number' && dir < GLib.UserDirectory.N_DIRECTORIES)
            places.push(GLib.get_user_special_dir(dir));
    }
    return places.filter(place => place && place !== '/');
}

export class IconFinder {
    constructor() {
        // The shell's icon theme, to ask whether a name is in it; it follows the icon theme setting by itself
        this._theme = new St.IconTheme();
        this._places = userPlaces();
        // The icon theme of each folder an item named, by folder: {stamp, theme}, theme being a promise of the icon
        // theme, or of null for a folder that is not searched
        this._folders = new Map();
        this._cancellable = new Gio.Cancellable();
    }

    destroy() {
        this._cancellable.cancel();
        this._folders = null;
        this._theme = null;
    }

    /**
     * The picture for one of an item's icons.
     *
     * @param {import('./item.js').Item} item - the tray item
     * @param {string} kind - '', 'Attention' or 'Overlay'
     * @param {object} look - how the icon is drawn
     * @param {number} look.size - in logical pixels
     * @param {number} look.scale - the shell's scale factor
     * @param {number} look.resourceScale - the actor's resource scale
     * @param {St.IconStyle} look.style - the panel's icon style
     * @param {boolean} look.rtl - whether the text runs right to left
     * @returns {Promise<{gicon: Gio.Icon}|{wide: St.ImageContent, width: number, height: number}|null>}
     */
    async find(item, kind, look) {
        const wanted = Math.ceil(look.size * look.scale * look.resourceScale);
        const name = this._cleanName(item.props[`${kind}IconName`]);
        if (name) {
            const found = await this._fromName(name, item.props.IconThemePath, wanted, look);
            if (found)
                return found;
        }

        const pixmapName = `${kind}IconPixmap`;
        const pixmaps = await item.pixmap(pixmapName);
        if (!pixmaps)
            return null;
        const content = pixmapContent(pixmaps, wanted);
        // Drawn once, the shell's copy of the pixels is enough until the size changes
        item.dropPixmap(pixmapName);
        return content ? {gicon: content} : null;
    }

    // Only a final .png or .svg goes: some programs give a file name for an icon name
    _cleanName(name) {
        if (typeof name !== 'string' || !name)
            return null;
        return name.startsWith('/') ? name : name.replace(IMAGE_SUFFIX, '');
    }

    // A picture, or a promise of one, for a name
    async _fromName(name, themePath, wanted, look) {
        if (name.startsWith('/'))
            return this._fromFile(name, wanted);

        if (typeof themePath === 'string' && themePath.startsWith('/') &&
            !this._theme.get_search_path().includes(themePath)) {
            const theme = await this._folderTheme(themePath);
            const file = theme ? this._lookInFolder(theme, name, look) : null;
            return file ? this._fromFile(file, wanted) : null;
        }

        return this._theme.has_icon(name) ? {gicon: new Gio.ThemedIcon({name})} : null;
    }

    // The item's own folder as an icon theme. Searching one reads the whole folder at once, on the shell's own
    // thread, so a folder is first looked at by Gio's worker threads: it has to be on one of this computer's own
    // disks (a network share that stops answering would stop the shell too) and to hold few entries. A folder that
    // passes keeps its icon theme, so that the next icon does not read it again, until its own date changes (a
    // program adds a file to it); that and the search itself, which stays on the main thread, only ever see a folder
    // that was small when it was looked at.
    async _folderTheme(folder) {
        const file = Gio.File.new_for_path(folder);
        let stamp = '';
        try {
            const info = await file.query_info_async('time::modified,time::modified-usec',
                Gio.FileQueryInfoFlags.NONE, GLib.PRIORITY_DEFAULT, this._cancellable);
            stamp = `${info.get_attribute_uint64('time::modified')}.${info.get_attribute_uint32('time::modified-usec')}`;
        } catch {
            // Not there: nothing to search, and the same answer until it appears and its date says so
        }
        if (!this._folders)
            return null;
        let kept = this._folders.get(folder);
        if (kept?.stamp !== stamp)
            kept = {stamp, theme: this._vet(file)};
        // Last asked for goes to the end of the map
        this._folders.delete(folder);
        this._folders.set(folder, kept);
        if (this._folders.size > FOLDERS_KEPT)
            this._folders.delete(this._folders.keys().next().value);
        return kept.theme;
    }

    async _vet(file) {
        try {
            const system = await file.query_filesystem_info_async('filesystem::remote,filesystem::type',
                GLib.PRIORITY_DEFAULT, this._cancellable);
            // FUSE is taken as remote too: whatever serves it may be a network, and may stop answering
            if (system.get_attribute_boolean('filesystem::remote') ||
                system.get_attribute_string('filesystem::type')?.startsWith('fuse'))
                return null;
            if (await entriesLeft(file, FOLDER_ENTRIES, FOLDER_DEPTH, this._cancellable) < 0)
                return null;
        } catch {
            return null;
        }
        const theme = new St.IconTheme();
        theme.set_search_path([file.get_path()]);
        return theme;
    }

    _lookInFolder(theme, name, look) {
        let flags = St.IconLookupFlags.GENERIC_FALLBACK;
        if (look.style === St.IconStyle.REGULAR)
            flags |= St.IconLookupFlags.FORCE_REGULAR;
        else if (look.style === St.IconStyle.SYMBOLIC)
            flags |= St.IconLookupFlags.FORCE_SYMBOLIC;
        flags |= look.rtl ? St.IconLookupFlags.DIR_RTL : St.IconLookupFlags.DIR_LTR;
        const scale = Math.max(1, Math.ceil(look.scale * look.resourceScale));
        // The one step of the search that cannot leave the main thread; the folder was checked to be small and local
        const info = theme.lookup_icon_for_scale(name, look.size, scale, flags);
        return info?.get_filename() ?? null;
    }

    async _fromFile(path, wanted) {
        const file = Gio.File.new_for_path(path);
        const canonical = file.get_path();
        const mine = this._places.some(place => canonical === place || canonical.startsWith(`${place}/`));
        // A symbolic file is left to the shell, which colours it like the panel's text
        if (!mine || /-symbolic(\.symbolic)?\.(svg|png)$/.test(canonical))
            return {gicon: new Gio.FileIcon({file})};

        let bytes;
        try {
            // A file somebody else may write: an ordinary one, and no bigger than an icon needs to be
            const info = await file.query_info_async('standard::type,standard::size', Gio.FileQueryInfoFlags.NONE,
                GLib.PRIORITY_DEFAULT, this._cancellable);
            if (info.get_file_type() !== Gio.FileType.REGULAR || info.get_size() > FILE_BYTES)
                return null;
            bytes = await readFile(file);
        } catch {
            return null;
        }
        // A PNG within FILE_SIDE, or an SVG; no other format has its size known before it is unpacked
        const natural = pngSize(bytes);
        if (natural ? natural.width > FILE_SIDE || natural.height > FILE_SIDE : !looksLikeSvg(bytes))
            return null;
        let pixbuf;
        try {
            pixbuf = await decodeScaled(bytes, wanted);
        } catch {
            return null;
        }
        const decoded = fromPixbuf(pixbuf);
        if (!decoded)
            return null;
        return decoded.wide ? {wide: decoded.content, width: decoded.width, height: decoded.height}
            : {gicon: decoded.content};
    }
}
