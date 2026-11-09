// The shell's theme made again with every stylesheet it had, less those to drop and with those to add, and put in place in
// one go, as GNOME Shell itself does when its theme changes (Main.loadTheme): loading or unloading a single stylesheet
// restyles the whole stage (some 50 ms each on a laptop's GPU), so this is one restyle whatever the number of files.
// `add` is a list of [name, Gio.File]; gives back those it added.
//
// St matches every widget against every stylesheet it holds each time a widget is styled, and pays for each file as well
// as for each rule: some 30 µs a widget for every file, which made opening the quick settings cost a third more with the
// modules' and the theme's 76 files than with the same rules in one. So the files given here are handed to St as one file,
// written into the runtime folder whenever the set changes, which holds their text in the order that keeps St's
// precedence: of two rules as specific as each other St takes the one from the file loaded first, so that file's text
// comes last.
//
// St also keeps every style it has worked out, for as long as its theme stays the same, and works a style out again only
// for a widget in a state it has not seen: a new theme makes every menu, the overview and the bar slow the first time each
// is drawn. So a lock leaves the theme as it is (`keep`): every rule here is tied to a class the modules take off when
// they stop, so none of it reaches the lock screen, and an unlock that brings back the same text changes nothing.

import GLib from 'gi://GLib';
import Gio from 'gi://Gio';
import St from 'gi://St';

// While the extension starts or stops (extension.js), what each caller drops and adds is gathered and put in place in one
// rebuild at the end: the theme's files and the modules' together, one restyle at a lock and one at an unlock
let batch = null;

// The files given here, in the order they were added; the one file St holds for them and its text; and whether St still
// holds it for a lock that kept the theme
let ours = [];
let merged = null;
let mergedText = null;
let kept = false;
let generation = 0;

// keep: when the batch leaves none of the files given here, St keeps its theme as it is (see above)
export function batchStylesheets(work, {keep = false} = {}) {
    batch = {drop: [], add: []};
    try {
        work();
    } finally {
        const {drop, add} = batch;
        batch = null;
        if (drop.length || add.length)
            replaceStylesheets(drop, add, {keep});
    }
}

const read = file => new TextDecoder().decode(file.load_contents(null)[1]);

// The text of every file given here, the one loaded first last; a file that cannot be read is left out
function mergedTextOf(files) {
    const parts = [];
    for (const file of [...files].reverse()) {
        try {
            // Each file is closed off from the next, so that one left open cannot swallow the rules that follow
            parts.push(`/* ${file.get_path()} */\n${read(file)}\n/* */\n`);
        } catch (e) {
            console.warn(`Amethystora UI: ${file.get_path()} was not loaded again: ${e.message}`);
        }
    }
    return parts.join('');
}

// What a shell that ended without taking its file away left behind
function sweep(dir, pid) {
    try {
        for (const info of Gio.File.new_for_path(dir).enumerate_children('standard::name', 0, null)) {
            const owner = /^shell-stylesheets-(\d+)-\d+\.css$/.exec(info.get_name())?.[1];
            if (owner && Number(owner) !== pid && !GLib.file_test(`/proc/${owner}`, GLib.FileTest.EXISTS))
                Gio.File.new_for_path(GLib.build_filenamev([dir, info.get_name()])).delete(null);
        }
    } catch {}
}

// A file of its own for the text, or null when it could not be written
function write(text) {
    const dir = GLib.build_filenamev([GLib.get_user_runtime_dir(), 'amethystora']);
    const pid = new Gio.Credentials().get_unix_pid();
    const path = GLib.build_filenamev([dir, `shell-stylesheets-${pid}-${++generation}.css`]);
    try {
        GLib.mkdir_with_parents(dir, 0o700);
        if (generation === 1)
            sweep(dir, pid);
        GLib.file_set_contents(path, text);
        return Gio.File.new_for_path(path);
    } catch (e) {
        console.warn(`Amethystora UI: the stylesheets could not be put together: ${e.message}`);
        return null;
    }
}

const listed = (file, files) => files.some(each => each.equal(file));

export function replaceStylesheets(drop, add, {keep = false} = {}) {
    // In a batch every file is taken as added: one that fails to load is said in the journal when the batch ends, and
    // dropping it later does nothing
    if (batch) {
        batch.drop.push(...drop);
        batch.add.push(...add);
        return add;
    }
    ours = ours.filter(file => !listed(file, drop));
    const added = [];
    for (const [name, file] of add) {
        if (file.query_exists(null)) {
            ours.push(file);
            added.push([name, file]);
        } else {
            console.warn(`Amethystora UI: ${name} was not loaded: ${file.get_path()} is not there`);
        }
    }

    const context = St.ThemeContext.get_for_stage(global.stage);
    const old = context.get_theme();
    const holds = merged && listed(merged, old.get_custom_stylesheets());
    // A lock: the theme stays as it is, and the files with it, until the unlock says what it wants
    if (keep && !ours.length && holds) {
        kept = true;
        return added;
    }
    const text = ours.length ? mergedTextOf(ours) : null;
    // The same text as St holds already, and nothing else to drop: nothing to do, and every style St knows stays
    if (text !== null && text === mergedText && holds && !old.get_custom_stylesheets().some(file => listed(file, drop))) {
        kept = false;
        return added;
    }
    kept = false;

    const theme = new St.Theme({
        application_stylesheet: old.application_stylesheet,
        theme_stylesheet: old.theme_stylesheet,
        default_stylesheet: old.default_stylesheet,
    });
    const previous = merged;
    merged = text !== null ? write(text) : null;
    mergedText = merged ? text : null;
    let placed = false;
    const place = () => {
        if (placed || !ours.length)
            return;
        placed = true;
        try {
            if (!merged)
                throw new Error('no file to load');
            theme.load_stylesheet(merged);
        } catch (e) {
            // Each file on its own, as before, so that one that St cannot read leaves the others
            console.warn(`Amethystora UI: the stylesheets were not loaded together: ${e.message}`);
            merged = null;
            mergedText = null;
            for (const file of ours) {
                try {
                    theme.load_stylesheet(file);
                } catch (error) {
                    console.warn(`Amethystora UI: ${file.get_path()} was not loaded: ${error.message}`);
                }
            }
        }
    };
    for (const file of old.get_custom_stylesheets()) {
        // Ours, merged or not, go where the first of them was, once
        if ((previous && file.equal(previous)) || listed(file, ours) || listed(file, drop)) {
            if (!listed(file, drop))
                place();
            continue;
        }
        // Another extension's stylesheet that has gone from the disk since it was loaded is left out, not fatal
        try {
            theme.load_stylesheet(file);
        } catch (e) {
            console.warn(`Amethystora UI: ${file.get_path()} was not loaded again: ${e.message}`);
        }
    }
    place();
    context.set_theme(theme);
    // Each set has a file of its own, so the last one is not needed once St holds the new one
    if (previous) {
        try {
            previous.delete(null);
        } catch {}
    }
    return added;
}
