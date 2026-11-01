// Modified for Amethystora: the history is kept sealed (store.js), never in plain text, and pictures are
// shown from memory instead of from files in ~/.cache.

import Cogl from 'gi://Cogl';
import GdkPixbuf from 'gi://GdkPixbuf';
import GLib from 'gi://GLib';
import Gio from 'gi://Gio';
import St from 'gi://St';
import { PrefsFields } from './constants.js';
import * as Keyring from './keyring.js';
import { Store, survivors, readOldCache, forgetOldCache } from './store.js';

// What the journal says when the history's state changes, once each time it does
const STATES = {
    open: 'the history is open, sealed under the login keyring',
    locked: 'the login keyring is locked: what was copied before waits for it to be unlocked',
    none: 'there is no keyring: the history is kept in memory only',
};
// How long the keyring may take to answer before the history is kept in memory and asked again later
const KEYRING_TIMEOUT = 20000;

export class Registry {
    #said = null;

    constructor ({ settings, uuid }) {
        this.uuid = uuid;
        this.settings = settings;
        this.store = new Store();
    }

    // open, locked (the keyring is), none (there is no keyring) or closed
    get state () {
        return this.store.state;
    }

    async open () {
        let timer = 0;
        try {
            await Promise.race([
                this.store.open(Keyring.find),
                new Promise((resolve, reject) => {
                    timer = setTimeout(() => reject(new Error(`no answer in ${KEYRING_TIMEOUT / 1000} s`)), KEYRING_TIMEOUT);
                }),
            ]);
        } catch (e) {
            console.warn(`Amethystora Clipboard: the keyring did not answer: ${e.message}`);
            if (this.store.state !== 'open') this.store.state = 'none';
        } finally {
            clearTimeout(timer);
        }
        if (this.store.state !== this.#said && STATES[this.store.state]) {
            this.#said = this.store.state;
            console.log(`Amethystora Clipboard: ${STATES[this.store.state]}`);
        }
    }

    unlock () {
        return Keyring.unlock();
    }

    watch (callback) {
        return Keyring.watch(callback);
    }

    write (entries) {
        if (this.store.state !== 'open') return Promise.resolve();

        const registryContent = [];

        for (let entry of entries) {
            const item = {
                favorite: entry.isFavorite(),
                mimetype: entry.mimetype()
            };

            registryContent.push(item);

            if (entry.isText()) {
                item.contents = entry.getStringValue();
            }
            else if (entry.isImage()) {
                item.contents = this.#sealImage(entry);
            }

            if (entry.getTag()) item.tag = entry.getTag();
        }

        return this.store.writeHistory(registryContent);
    }

    // A picture is sealed once, under a random name: a hash of its bytes would let anyone confirm that a
    // picture they know was copied
    #sealImage (entry) {
        entry.imageId ??= Store.newId();
        if (!entry.imageSealed) {
            entry.imageSealed = true;
            this.store.sealImage(entry.imageId, entry.asBytes().get_data() ?? new Uint8Array()).catch(() => {
                entry.imageSealed = false;
            });
        }
        return entry.imageId;
    }

    // The history as kept; with prune, the pictures it does not name go. Only before anything else can seal
    // one, that is at the start: a picture copied meanwhile would go too
    async read ({ prune = false } = {}) {
        if (this.store.state !== 'open') return [];

        const max_size = this.settings.get_int(PrefsFields.HISTORY_SIZE);
        const history = await this.store.readHistory(this.settings.get_int(PrefsFields.CACHE_FILE_SIZE) * 1024 * 1024);
        const kept = survivors(history, this.settings.get_string(PrefsFields.KEEP_HISTORY));
        const old = await readOldCache();

        const clipboardEntries = [];
        for (const jsonEntry of [...(old ?? []), ...kept]) {
            const entry = await ClipboardEntry.fromJSON(jsonEntry, this.store);
            if (entry && !clipboardEntries.some(other => other.equals(entry)))
                clipboardEntries.push(entry);
        }

        let registryNoFavorite = clipboardEntries
            .filter(entry => !entry.isFavorite());

        while (registryNoFavorite.length > max_size) {
            let oldestNoFavorite = registryNoFavorite.shift();
            let itemIdx = clipboardEntries.indexOf(oldestNoFavorite);
            clipboardEntries.splice(itemIdx,1);

            registryNoFavorite = clipboardEntries.filter(
                entry => !entry.isFavorite()
            );
        }

        // What keep-history dropped goes now, the pictures with it, and what Clipboard Indicator left once it
        // is sealed
        if (old || kept.length !== (history?.entries?.length ?? 0) || clipboardEntries.length !== kept.length) {
            try {
                await this.write(clipboardEntries);
                if (old) forgetOldCache();
            } catch (e) {
                console.warn(`Amethystora Clipboard: the history was not written: ${e.message}`);
            }
        }
        if (prune) this.prune(clipboardEntries);

        return clipboardEntries;
    }

    // Every sealed picture but those of these entries. Called right after a write of them, so that a picture
    // copied since was sealed under a name it knows
    prune (entries) {
        this.store.prune(entries.map(entry => entry.imageId).filter(Boolean));
    }

    async getEntryAsImage (entry) {
        if (entry.isImage() === false) return;

        return new St.Icon({ gicon: Gio.BytesIcon.new(entry.asBytes()) });
    }

    // The picture in a square of size pixels, its proportions kept
    // The picture at its own proportions, as large as fits in width × height and never larger than it is; null for one
    // that is not a picture or does not read
    getEntryAsPicture (entry, width, height) {
        if (entry.isImage() === false) return null;
        let pixbuf;
        try {
            const stream = Gio.MemoryInputStream.new_from_bytes(entry.asBytes());
            pixbuf = GdkPixbuf.Pixbuf.new_from_stream(stream, null);
            stream.close(null);
        } catch {
            return null;
        }
        const scale = Math.min(width / pixbuf.get_width(), height / pixbuf.get_height(), 1);
        const w = Math.max(1, Math.round(pixbuf.get_width() * scale));
        const h = Math.max(1, Math.round(pixbuf.get_height() * scale));
        pixbuf = pixbuf.scale_simple(w, h, GdkPixbuf.InterpType.BILINEAR);
        if (!pixbuf.get_has_alpha())
            pixbuf = pixbuf.add_alpha(false, 0, 0, 0);
        const context = global.stage.context.get_backend().get_cogl_context();
        const content = St.ImageContent.new_with_preferred_size(w, h);
        content.set_bytes(context, pixbuf.read_pixel_bytes(), Cogl.PixelFormat.RGBA_8888, w, h, pixbuf.get_rowstride());
        return new St.Widget({ content, width: w, height: h });
    }

    async deleteEntryFile (entry) {
        if (!entry.imageId) return;

        entry.imageSealed = false;
        try {
            await this.store.deleteImage(entry.imageId);
        }
        catch (e) {
            console.error(e);
        }
    }

    clearCacheFolder() {
        this.store.clear();
    }
}

export class ClipboardEntry {
    #mimetype;
    #bytes;
    #favorite;

    // Its sealed file's name, once it has one
    imageId = null;
    imageSealed = false;

    static __isText (mimetype) {
        return mimetype.startsWith('text/') ||
            mimetype === 'STRING' ||
            mimetype === 'UTF8_STRING';
    }

    // An entry of the history as written, or of Clipboard Indicator's with its picture's bytes
    static async fromJSON (jsonEntry, store) {
        const mimetype = jsonEntry.mimetype || 'text/plain;charset=utf-8';
        const favorite = !!jsonEntry.favorite;
        let bytes;
        let imageId = null;

        if (ClipboardEntry.__isText(mimetype)) {
            if (typeof jsonEntry.contents !== 'string') return null;
            bytes = new TextEncoder().encode(jsonEntry.contents);
        }
        else if (mimetype.startsWith('image/')) {
            if (jsonEntry.bytes) {
                bytes = jsonEntry.bytes;
            }
            else {
                bytes = await store.openImage(jsonEntry.contents);
                imageId = jsonEntry.contents;
            }
            if (!bytes) return null;
        }
        else {
            return null;
        }

        const entry = new ClipboardEntry(mimetype, bytes, favorite);
        if (imageId) {
            entry.imageId = imageId;
            entry.imageSealed = true;
        }
        if (jsonEntry.tag) entry.setTag(jsonEntry.tag);
        return entry;
    }

    constructor (mimetype, bytes, favorite) {
        this.#mimetype = mimetype;
        this.#bytes = bytes;
        this.#favorite = favorite;
    }

    getStringValue () {
        if (this.isImage()) {
            return `[Image ${this.asBytes().hash()}]`;
        }
        return new TextDecoder().decode(this.#bytes);
    }

    mimetype () {
        return this.#mimetype;
    }

    isFavorite () {
        return this.#favorite;
    }

    set favorite (val) {
        this.#favorite = !!val;
    }

    isText () {
        return ClipboardEntry.__isText(this.#mimetype);
    }

    isImage () {
        return this.#mimetype.startsWith('image/');
    }

    setText (text) {
        if (!this.isText()) return;
        this.#bytes = new TextEncoder().encode(text);
    }

    #tag = null;

    getTag () {
        return this.#tag;
    }

    setTag (tag) {
        this.#tag = tag || null;
    }

    asBytes () {
        return GLib.Bytes.new(this.#bytes);
    }

    equals (otherEntry) {
        return this.getStringValue() === otherEntry.getStringValue();
        // this.asBytes().equal(otherEntry.asBytes());
    }
}
