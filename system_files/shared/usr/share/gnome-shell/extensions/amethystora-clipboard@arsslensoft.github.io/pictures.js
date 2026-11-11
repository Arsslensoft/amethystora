// What the history takes of a copy, judged before anything decodes it. GdkPixbuf here hands pictures to glycin, which
// decodes them whole, so a small file that declares 30000 x 30000 would make gigabytes inside the shell: the size is
// read from the header (as Amethystora UI's tray does for its icons) and a picture or text over the ceilings is
// skipped. No import here: build_files/shared/test-clipboard-pictures.mjs runs it under node.

// The most a copy may weigh to be recorded, and the most a picture may declare (36 million pixels is a 8K screen)
export const MAX_PICTURE_BYTES = 32 * 1024 * 1024;
export const MAX_TEXT_BYTES = 8 * 1024 * 1024;
export const MAX_PICTURE_PIXELS = 36 * 1000 * 1000;
export const MAX_PICTURE_SIDE = 16384;

// The width and height a picture's header declares, from its magic bytes (never from the type the app said), or null
// for data that is no PNG, GIF, JPEG, WebP or SVG that can be read
export function pictureSize (data) {
    const n = data.length;
    const u16be = at => (data[at] << 8) | data[at + 1];
    const u16le = at => data[at] | (data[at + 1] << 8);
    const u24le = at => data[at] | (data[at + 1] << 8) | (data[at + 2] << 16);
    const tag = (at, text) => n >= at + text.length && [...text].every((c, i) => data[at + i] === c.charCodeAt(0));

    if (n >= 24 && data[0] === 0x89 && tag(1, 'PNG') && tag(12, 'IHDR')) {
        const word = at => ((data[at] << 24) | (data[at + 1] << 16) | (data[at + 2] << 8) | data[at + 3]) >>> 0;
        return {width: word(16), height: word(20)};
    }
    if (n >= 10 && (tag(0, 'GIF87a') || tag(0, 'GIF89a')))
        return {width: u16le(6), height: u16le(8)};
    if (n >= 4 && data[0] === 0xff && data[1] === 0xd8) {
        let at = 2;
        while (at + 9 < n) {
            if (data[at] !== 0xff) { at++; continue; }
            const marker = data[at + 1];
            if (marker === 0xff) { at++; continue; }
            // Markers without a length
            if (marker === 0x01 || (marker >= 0xd0 && marker <= 0xd9)) { at += 2; continue; }
            // A frame header: every SOF but DHT (c4), JPG (c8) and DAC (cc)
            if (marker >= 0xc0 && marker <= 0xcf && marker !== 0xc4 && marker !== 0xc8 && marker !== 0xcc)
                return {width: u16be(at + 7), height: u16be(at + 5)};
            at += 2 + u16be(at + 2);
        }
        return null;
    }
    if (n >= 30 && tag(0, 'RIFF') && tag(8, 'WEBP')) {
        if (tag(12, 'VP8X'))
            return {width: u24le(24) + 1, height: u24le(27) + 1};
        if (tag(12, 'VP8L') && data[20] === 0x2f) {
            const bits = (data[21] | (data[22] << 8) | (data[23] << 16) | (data[24] << 24)) >>> 0;
            return {width: (bits & 0x3fff) + 1, height: ((bits >>> 14) & 0x3fff) + 1};
        }
        if (tag(12, 'VP8 '))
            return {width: u16le(26) & 0x3fff, height: u16le(28) & 0x3fff};
        return null;
    }
    // A vector picture is drawn at the size it declares: read the root's width and height (a viewBox when there is
    // none); a compressed one is never read. Only text is looked at, and <svg> must be the document's root element (after
    // an XML declaration, comments and a DOCTYPE): a picture of another format that holds the word early is no SVG
    let head;
    try {
        head = new TextDecoder('utf-8', {fatal: true}).decode(data.subarray(0, Math.min(n, 4096)), {stream: true});
    } catch {
        return null;
    }
    if (/[\u0000-\u0008\u000b\u000e-\u001f]/.test(head))
        return null;
    const found = /^\s*(?:<\?xml[^>]*\?>\s*|<!--[\s\S]*?-->\s*|<!DOCTYPE[^>[]*(?:\[[\s\S]*?\]\s*)?>\s*)*(<svg\b[^>]*>)/i.exec(head);
    const root = found ? [found[1]] : null;
    if (root) {
        const attr = name => {
            const m = new RegExp(`\\s${name}\\s*=\\s*["']\\s*([0-9.]+)\\s*(px|pt|mm|cm|in|pc|em|ex|%)?\\s*["']`, 'i').exec(root[0]);
            if (!m) return null;
            const scale = {pt: 4 / 3, pc: 16, mm: 96 / 25.4, cm: 96 / 2.54, in: 96, em: 16, ex: 8}[(m[2] ?? '').toLowerCase()] ?? 1;
            return m[2] === '%' ? null : Number(m[1]) * scale;
        };
        let width = attr('width');
        let height = attr('height');
        if (width === null || height === null) {
            const box = /\sviewBox\s*=\s*["']\s*[-0-9.e]+[\s,]+[-0-9.e]+[\s,]+([0-9.e]+)[\s,]+([0-9.e]+)/i.exec(root[0]);
            width ??= box ? Number(box[1]) : 0;
            height ??= box ? Number(box[2]) : 0;
        }
        return {width: Math.ceil(width) || 0, height: Math.ceil(height) || 0};
    }
    return null;
}

// Whether a picture is small enough to be recorded and decoded: a header that cannot be read, or that declares more
// than the ceilings, says no
export function pictureFits (data) {
    if (!data || data.length === 0 || data.length > MAX_PICTURE_BYTES)
        return false;
    const size = pictureSize(data);
    if (!size || !(size.width > 0) || !(size.height > 0))
        return false;
    return size.width <= MAX_PICTURE_SIDE && size.height <= MAX_PICTURE_SIDE &&
        size.width * size.height <= MAX_PICTURE_PIXELS;
}

// Whether a copy of this type and size is small enough to be recorded at all
export function copyFits (isImage, data) {
    return isImage ? pictureFits(data) : data.length <= MAX_TEXT_BYTES;
}
