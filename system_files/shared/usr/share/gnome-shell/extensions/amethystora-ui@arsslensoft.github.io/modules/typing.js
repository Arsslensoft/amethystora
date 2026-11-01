// Text shortcuts: type a shortcut of your own where a word starts, then a space, in an app that uses GNOME's input
// method, and it becomes its text (;addr and a space becomes your address). Off until the person turns it on (the key
// expand, ame desktop text-shortcuts on, Control's Typing group): it looks at what is typed, so it is their choice.
// The shortcuts are ~/.config/amethystora/text-shortcuts.json, which only /usr/libexec/amethystora-text-shortcuts
// writes; this module only reads it, while the switch is on, and again when it changes.
//
// Off, nothing is held or watched: no wrapper on the input method, no file monitor, no signal but the switch's own.
// On, nothing runs until a key is typed into an app (typing/expand.js).
//
// Hold a key for accents (the key accents, offered and off: ame desktop accents on, Control's Keyboard page) is here
// too, at the same place in the shell (typing/accents.js): a letter held shows its accented forms over the cursor.

import Gio from 'gi://Gio';
import GLib from 'gi://GLib';

import {Accents} from './typing/accents.js';
import {Expander, inputMethodFits} from './typing/expand.js';
import {LIMITS, parseShortcuts} from './typing/match.js';

const SCHEMA = 'org.gnome.shell.extensions.amethystora-ui.typing';
const DIR = GLib.build_filenamev([GLib.get_user_config_dir(), 'amethystora']);
const FILE = GLib.build_filenamev([DIR, 'text-shortcuts.json']);
const COPY = `${FILE}.bak`;
// Several changes in a row (the file and its copy) are read once
const SETTLE = 200;

// One file's shortcuts, read away from the shell's main loop, then done(result): a link is never followed, the file is
// opened in a worker thread (a pipe swapped in meanwhile holds that thread, never the shell), and what was opened is
// checked again by its descriptor to be a plain file of the helper's size before anything of it is read
function readShortcuts(path, cancellable, done) {
    const none = () => done(parseShortcuts(null));
    const file = Gio.File.new_for_path(path);
    file.query_info_async('standard::type', Gio.FileQueryInfoFlags.NOFOLLOW_SYMLINKS, GLib.PRIORITY_DEFAULT,
        cancellable, (_f, queried) => {
            let info;
            try {
                info = file.query_info_finish(queried);
            } catch {
                none();
                return;
            }
            if (info.get_file_type() !== Gio.FileType.REGULAR) {
                none();
                return;
            }
            file.read_async(GLib.PRIORITY_DEFAULT, cancellable, (_f2, opened) => {
                let stream;
                try {
                    stream = file.read_finish(opened);
                } catch {
                    none();
                    return;
                }
                const finish = result => {
                    stream.close_async(GLib.PRIORITY_DEFAULT, null, null);
                    done(result);
                };
                stream.query_info_async('standard::type,standard::size', GLib.PRIORITY_DEFAULT, cancellable,
                    (_s, described) => {
                        let fd;
                        try {
                            fd = stream.query_info_finish(described);
                        } catch {
                            finish(parseShortcuts(null));
                            return;
                        }
                        if (fd.get_file_type() !== Gio.FileType.REGULAR || fd.get_size() > LIMITS.fileMax) {
                            finish(parseShortcuts(null));
                            return;
                        }
                        stream.read_bytes_async(LIMITS.fileMax + 1, GLib.PRIORITY_DEFAULT, cancellable, (_s2, read) => {
                            let bytes = null;
                            try {
                                bytes = stream.read_bytes_finish(read);
                            } catch {}
                            if (!bytes || bytes.get_size() > LIMITS.fileMax)
                                finish(parseShortcuts(null));
                            else
                                finish(parseShortcuts(new TextDecoder().decode(bytes.toArray())));
                        });
                    });
            });
        });
}

export class Typing {
    constructor(ext) {
        this._ext = ext;
    }

    enable() {
        // What the tests ask (snapshot()): counts only, never anything typed
        this._counts = {
            observed: 0, expansions: 0, errors: 0, maxUs: 0, totalUs: 0, timed: 0, expandMaxUs: 0,
            skipped: {password: 0, purpose: 0, shell: 0, preedit: 0, filtered: 0, terminal: 0, unseen: 0},
            accentsOpened: 0, accentsPicked: 0, accentsGaveUp: 0, accentsNoCaret: 0,
        };
        this._settings = this._ext.getSettings(SCHEMA);
        this._settings.connectObject('changed::expand', () => this._sync(), 'changed::accents',
            () => this._syncAccents(), this);
        // The accents first, so that the shortcuts' observer, put on top of them, sees what the app sees
        this._syncAccents();
        this._sync();
    }

    disable() {
        this._settings?.disconnectObject(this);
        this._stop();
        this._stopAccents();
        // Disposed now rather than by the collector (dock.js says why)
        this._settings?.run_dispose();
        this._settings = null;
    }

    _sync() {
        if (this._settings.get_boolean('expand'))
            this._start();
        else
            this._stop();
    }

    _syncAccents() {
        if (!this._settings.get_boolean('accents')) {
            this._stopAccents();
            return;
        }
        if (this._accents)
            return;
        if (!inputMethodFits()) {
            if (!this._unfit)
                console.warn('Amethystora UI: accents: GNOME\'s input method changed');
            this._unfit = true;
            return;
        }
        this._accents = new Accents(this._counts);
        try {
            this._accents.start();
        } catch (e) {
            console.error(`Amethystora UI: accents did not start: ${e.message}`);
            this._stopAccents();
        }
    }

    _stopAccents() {
        this._accents?.stop();
        this._accents = null;
    }

    _start() {
        if (this._expander)
            return;
        if (!inputMethodFits()) {
            if (!this._unfit)
                console.warn('Amethystora UI: text shortcuts: GNOME\'s input method changed');
            this._unfit = true;
            return;
        }
        this._expander = new Expander(this._counts);
        try {
            this._expander.start();
            this._watchFile();
            this._reload();
        } catch (e) {
            console.error(`Amethystora UI: text shortcuts did not start: ${e.message}`);
            this._stop();
        }
    }

    _stop() {
        // A read on its way is cancelled; until it has finished (a worker held by a pipe may never), none other starts
        this._cancellable?.cancel();
        this._cancellable = null;
        this._again = false;
        this._expander?.stop();
        this._expander = null;
        this._monitor?.disconnectObject(this);
        this._monitor?.cancel();
        this._monitor = null;
        if (this._settle) {
            GLib.source_remove(this._settle);
            this._settle = 0;
        }
    }

    // The file's shortcuts, or its copy's when the file is not a shortcuts file at all (as the helper reads them). One
    // read at a time: a change while one is on its way reads again once it has finished, so that a file swapped for
    // something that never answers holds at most one of GIO's worker threads, never one for each change. A read the
    // module stopped meanwhile is let go
    _reload() {
        if (this._reading) {
            this._again = true;
            return;
        }
        this._reading = true;
        this._again = false;
        const cancellable = new Gio.Cancellable();
        this._cancellable = cancellable;
        const loaded = result => {
            this._reading = false;
            if (this._cancellable === cancellable)
                this._cancellable = null;
            if (!cancellable.is_cancelled())
                this._expander?.setShortcuts(result);
            if (this._again && this._expander)
                this._reload();
        };
        readShortcuts(FILE, cancellable, result => {
            if (result.ok || cancellable.is_cancelled())
                loaded(result);
            else
                readShortcuts(COPY, cancellable, copy => loaded(copy.ok ? copy : result));
        });
    }

    _watchFile() {
        // The folder is there in every account that chose a theme; made here otherwise, so that the monitor watches it
        // instead of looking for it every few seconds
        if (!GLib.file_test(DIR, GLib.FileTest.IS_DIR))
            GLib.mkdir_with_parents(DIR, 0o700);
        this._monitor = Gio.File.new_for_path(FILE).monitor_file(Gio.FileMonitorFlags.NONE, null);
        this._monitor.connectObject('changed', () => {
            if (this._settle)
                GLib.source_remove(this._settle);
            this._settle = GLib.timeout_add(GLib.PRIORITY_DEFAULT, SETTLE, () => {
                this._settle = 0;
                this._reload();
                return GLib.SOURCE_REMOVE;
            });
        }, this);
    }

    // For the tests: what it is doing, in counts. Nothing typed and no shortcut's text is ever in it
    snapshot() {
        const counts = this._counts;
        return JSON.stringify({
            on: !!this._settings?.get_boolean('expand'),
            wrapped: !!this._expander?.wrapped,
            observed: counts.observed,
            bufferLength: this._expander?.bufferLength ?? 0,
            expansions: counts.expansions,
            skipped: {...counts.skipped},
            errors: counts.errors,
            // The longest a key took, and on average; the space that starts an expansion apart
            maxUs: counts.maxUs,
            meanUs: counts.timed ? Math.round(counts.totalUs / counts.timed) : 0,
            timed: counts.timed,
            totalUs: counts.totalUs,
            expandMaxUs: counts.expandMaxUs,
            monitor: !!this._monitor,
            shortcuts: this._expander?.count ?? 0,
            unfit: !!this._unfit,
            accents: {
                on: !!this._settings?.get_boolean('accents'), wrapped: !!this._accents?.wrapped,
                open: !!this._accents?.open, opened: counts.accentsOpened, picked: counts.accentsPicked,
                gaveUp: counts.accentsGaveUp, noCaret: counts.accentsNoCaret,
            },
        });
    }
}
