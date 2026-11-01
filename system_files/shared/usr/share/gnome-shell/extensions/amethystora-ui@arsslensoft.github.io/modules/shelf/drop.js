// What a window's drag holds, read by the shell while the drag hovers the shelf's glow or card, and handed to the
// Shelf app on its standard input (plans/beyond/spec-r4-shelf.md, R4-08 section 2).
//
// A drag let go over the shell goes to nobody, and Mutter forgets its data before the shell hears that it ended, so the
// data is read while the drag still holds it, as the dock reads the files dragged over it (dock/drops.js). Only one
// kind is taken from a drag: local files, else a PNG picture, else text. Nothing read is logged, written or put on a
// command line: it goes to the program on a pipe, and the shell lets go of it then.

import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import Meta from 'gi://Meta';

import {parseUriList} from '../dock/drops.js';

// The most of a list of files or of text, and of a picture, in bytes: the program's own limits (apps/shelf/src/model.rs
// MOST_TEXT and MOST_PICTURE, which 20-tests.sh holds to these)
export const MOST_TEXT = 1 << 20;
export const MOST_PICTURE = 32 << 20;
// Text in UTF-8 only: Latin-1's STRING would read as something else
const TEXT_TYPES = ['text/plain;charset=utf-8', 'text/plain', 'UTF8_STRING'];
export const PROGRAM = '/usr/bin/amethystora-shelf';

// Text as UTF-8, or '' for bytes that are not: never a character put in place of another, which would change its length
function utf8(bytes) {
    try {
        return new TextDecoder('utf-8', {fatal: true}).decode(bytes);
    } catch {
        return '';
    }
}

function transfer(selection, type, most, cancellable) {
    return new Promise(resolve => {
        const stream = Gio.MemoryOutputStream.new_resizable();
        // One byte over the most, so that a drop that is too large is seen to be and refused
        selection.transfer_async(Meta.SelectionType.SELECTION_DND, type, most + 1, stream, cancellable, (o, result) => {
            try {
                selection.transfer_finish(result);
                stream.close(null);
                const bytes = stream.steal_as_bytes().toArray();
                resolve(bytes.length > 0 && bytes.length <= most ? bytes : null);
            } catch {
                resolve(null);
            }
        });
    });
}

// The drag's data as the shelf takes it: {files: [uri...]} | {picture: Uint8Array} | {text: string}, {refused: true}
// for one it cannot take (too large, or empty), which the shelf then says, or null for a drag that offered nothing
export async function readDrag(cancellable) {
    const selection = global.display.get_selection();
    const types = selection.get_mimetypes(Meta.SelectionType.SELECTION_DND) ?? [];
    if (types.includes('text/uri-list')) {
        const bytes = await transfer(selection, 'text/uri-list', MOST_TEXT, cancellable);
        const files = bytes ? parseUriList(utf8(bytes)) : [];
        if (files.length)
            return {files: files.map(file => file.uri)};
    }
    if (types.includes('image/png')) {
        const picture = await transfer(selection, 'image/png', MOST_PICTURE, cancellable);
        if (picture)
            return {picture};
    }
    const textType = TEXT_TYPES.find(type => types.includes(type));
    if (textType) {
        const bytes = await transfer(selection, textType, MOST_TEXT, cancellable);
        const text = bytes ? utf8(bytes) : '';
        bytes?.fill(0);
        if (text.trim())
            return {text};
    }
    return types.length ? {refused: true} : null;
}

// The hand-off: `amethystora-shelf take LAYOUT` with the drop on its standard input, one line of JSON then the
// picture's bytes. Resolves once it is written, with the process id of the program started (0 when none); the program
// stays while the shelf holds something, and GIO reaps it. What the shell read is overwritten once it is written.
export function handOff(drop, layout) {
    let header;
    let tail = null;
    if (drop.refused)
        header = {items: []};
    else if (drop.files)
        header = {items: [{kind: 'files', uris: drop.files}]};
    else if (drop.picture)
        [header, tail] = [{items: [{kind: 'picture', mime: 'image/png', length: drop.picture.length}]}, drop.picture];
    else
        header = {items: [{kind: 'text', text: drop.text}]};
    const line = new TextEncoder().encode(`${JSON.stringify(header)}\n`);
    const all = new Uint8Array(line.length + (tail?.length ?? 0));
    all.set(line, 0);
    if (tail)
        all.set(tail, line.length);
    line.fill(0);
    tail?.fill(0);
    drop.text = drop.files = drop.picture = null;
    return new Promise(resolve => {
        try {
            const process = Gio.Subprocess.new([PROGRAM, 'take', layout === 'row' ? 'row' : 'column'],
                Gio.SubprocessFlags.STDIN_PIPE);
            const pid = Number(process.get_identifier() ?? 0);
            const stdin = process.get_stdin_pipe();
            stdin.write_all_async(all, GLib.PRIORITY_DEFAULT, null, (s, result) => {
                try {
                    s.write_all_finish(result);
                } catch (e) {
                    console.warn(`Amethystora UI: the shelf did not take a drop: ${e.message}`);
                }
                all.fill(0);
                // The end of the drop: the program reads to here
                s.close_async(GLib.PRIORITY_DEFAULT, null, (c, r) => {
                    try {
                        c.close_finish(r);
                    } catch {}
                    resolve(pid);
                });
            });
        } catch (e) {
            all.fill(0);
            console.warn(`Amethystora UI: the shelf could not start: ${e.message}`);
            resolve(0);
        }
    });
}
