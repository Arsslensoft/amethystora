// The theme's colours in the shell. amethystora-theme renders each /usr/share/amethystora/themed/shell-<module>.css.tpl
// from the palette into the current theme, and this loads every one of them over the modules' own stylesheets, which
// hold what never changes with the theme. They are loaded again whenever the theme changes. A theme can hand-write any
// of them.
//
// With an accent per workspace (ame desktop accent-per-workspace), amethystora-theme also renders, into accents/<id>/
// beside them, the rules each accent changes, under a class of the shell's top group; these are loaded with the theme's,
// and once a workspace switch has slid into place the shown workspace's class is put on the top group, and its accent
// written to ~/.local/state/amethystora/workspace-accent, which the Amethystora apps follow. A focus profile's accent
// (modules/focus.js, setFocusAccent) is put on the same way, over every workspace's, its rules rendered with the
// workspaces' (AMETHYSTORA_FOCUS_ACCENT). It also keeps the wallpaper's two other links to the theme up to date by asking
// amethystora-theme: colours from the wallpaper when the picture changes (in GNOME Settings, say), and Daylight's
// slideshow when Night Light's hours change or a session starts after the clocks changed.

import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import St from 'gi://St';

import * as Main from 'resource:///org/gnome/shell/ui/main.js';

import {IDS, SCHEMA, workspaceAccent} from './theme/accents.js';

const CONFIG = GLib.build_filenamev([GLib.get_user_config_dir(), 'amethystora']);
const CURRENT = GLib.build_filenamev([CONFIG, 'current']);
const THEME_DIR = GLib.build_filenamev([CURRENT, 'theme']);
const ACCENTS_DIR = GLib.build_filenamev([THEME_DIR, 'accents']);
const STATE_DIR = GLib.build_filenamev([GLib.get_user_state_dir(), 'amethystora']);
const STATE = GLib.build_filenamev([STATE_DIR, 'workspace-accent']);
const THEME_COMMAND = '/usr/bin/amethystora-theme';
// The way an image of one's own takes colours from the wallpaper (amethystora-setup bake), after the account's
const IMAGE_COLOURS = '/usr/share/amethystora/image-setup/colours';
const STYLESHEET = /^shell-[a-z]+\.css$/;
// Milliseconds for a theme switch (a folder taken away, another moved in, the name written) to settle
const SETTLE = 300;
// What a theme switch touches in the current folder: the theme's own folder and the file that names it. A new
// wallpaper touches others (the `background` link, `backgrounds/`), and none of those changes a stylesheet.
const UNTHEMED_LIGHT = 'amethystora-unthemed-light';
const SWITCH = ['theme', 'theme.name'];
// A workspace's accent is put on once its slide has ended, so that a run of switches restyles the shell once
const SLIDE = 300;
// How long a burst of changes to a key is let settle before amethystora-theme is asked: a new picture comes as a few
// keys at once, and Settings' hour spinners change Night Light's many times a second
const PICTURE_SETTLE = 1000;
const NIGHT_LIGHT_SETTLE = 2000;
const BACKGROUND = 'org.gnome.desktop.background';
const NIGHT_LIGHT = 'org.gnome.settings-daemon.plugins.color';
const NIGHT_LIGHT_KEYS = ['night-light-enabled', 'night-light-schedule-automatic', 'night-light-schedule-from',
    'night-light-schedule-to'];

export class Theme {
    constructor(ext) {
        this._ext = ext;
    }

    enable() {
        // What is loaded, by a stylesheet's name: the file in use, the theme's or an accent's
        this._loaded = new Map();
        this._accent = null;
        this._focusAccent = null;
        this._focusAsked = false;
        this._timeouts = new Map();
        this._runs = new Map();
        // The keys first, so that the first load already carries the accents' rules: one rebuild of the shell's theme
        this._settings = this._ext.getSettings(SCHEMA);
        this._load();
        St.Settings.get().connectObject('notify::color-scheme', () => this._markUnthemed(), this);
        try {
            this._monitor = Gio.File.new_for_path(CURRENT).monitor_directory(Gio.FileMonitorFlags.WATCH_MOVES, null);
            // A move names both of its ends (the folder it was built in, then the one it became)
            this._monitor.connect('changed', (monitor, file, other) => {
                if ([file, other].some(each => SWITCH.includes(each?.get_basename()))) {
                    this._later('switch', SETTLE, () => {
                        // A new theme: a focus profile's accent is rendered for it once more
                        this._focusAsked = false;
                        this._load();
                    });
                }
            });
        } catch (e) {
            // Before the first theme there is no folder to watch: the next login loads what the theme set
            console.warn(`Amethystora UI: ${CURRENT} cannot be watched: ${e.message}`);
        }

        this._settings.connectObject(
            'changed::accent-per-workspace', () => this._perWorkspaceChanged(),
            'changed::workspace-accents', () => this._accentsChanged(),
            this);
        this._perWorkspaceChanged();

        this._background = new Gio.Settings({schema_id: BACKGROUND});
        this._background.connectObject('changed::picture-uri', () => {
            // Only while colours are taken from the wallpaper
            if (this._coloursOn())
                this._later('colours', PICTURE_SETTLE, () => this._run('colours', ['colours', 'refresh']));
        }, this);
        if (Gio.SettingsSchemaSource.get_default().lookup(NIGHT_LIGHT, true)) {
            this._nightLight = new Gio.Settings({schema_id: NIGHT_LIGHT});
            for (const key of NIGHT_LIGHT_KEYS) {
                this._nightLight.connectObject(`changed::${key}`, () => {
                    if (this._daylightShown())
                        this._later('daylight', NIGHT_LIGHT_SETTLE, () => this._run('daylight', ['daylight', 'refresh']));
                }, this);
            }
        }
        // A session that starts (or comes back from the lock screen) after the clocks changed shows the slideshow an
        // hour off until it is written again
        if (this._daylightShown())
            this._run('daylight', ['daylight', 'refresh']);
    }

    disable() {
        St.Settings.get().disconnectObject(this);
        Main.uiGroup.remove_style_class_name(UNTHEMED_LIGHT);
        this._monitor?.cancel();
        this._monitor = null;
        for (const id of this._timeouts?.values() ?? [])
            GLib.source_remove(id);
        this._timeouts = null;
        for (const run of this._runs?.values() ?? [])
            run.cancellable.cancel();
        this._runs = null;
        global.workspace_manager.disconnectObject(this);
        this._workspaceHandler = false;
        this._settings?.disconnectObject(this);
        // Disposed now rather than by the collector (dock.js says why)
        this._settings?.run_dispose();
        this._settings = null;
        this._background?.disconnectObject(this);
        this._background?.run_dispose();
        this._background = null;
        this._nightLight?.disconnectObject(this);
        this._nightLight?.run_dispose();
        this._nightLight = null;
        if (this._prewarmId)
            GLib.source_remove(this._prewarmId);
        this._prewarmId = 0;
        if (this._accent)
            Main.uiGroup.remove_style_class_name(`amethystora-accent-${this._accent}`);
        // The apps go back to the theme's accent with the shell
        if (this._accent)
            this._writeState(null);
        this._unload();
        this._loaded = null;
        this._accent = null;
        this._focusAccent = null;
    }

    // A focus profile's accent (one of the twelve), or null: put on the top group over a workspace's, in memory, its
    // rules asked of amethystora-theme first when they are not rendered yet
    setFocusAccent(id) {
        id = IDS.includes(id) ? id : null;
        if (!this._loaded || id === this._focusAccent)
            return;
        this._focusAccent = id;
        this._focusAsked = false;
        if (id && !this._loaded.has(`accent-${id}`)) {
            const file = Gio.File.new_for_path(GLib.build_filenamev([ACCENTS_DIR, id, 'shell-accent.css']));
            if (file.query_exists(null)) {
                this._load();
            } else {
                this._focusAsked = true;
                this._run('accents', ['accents'], () => this._load());
            }
            return;
        }
        // Ending, the accents' rules stay loaded until the theme's are next loaded: a class swap, not a restyle
        this._apply();
    }

    get focusAccent() {
        return this._focusAccent ?? null;
    }

    // The workspace accent loaded now, or null for the theme's own (the workspaces' menu and the lab ask)
    get workspaceAccent() {
        return this._accent;
    }

    // Whether a workspace switch is being listened to, which only an accent per workspace does
    get listening() {
        return this._workspaceHandler ?? false;
    }

    _later(kind, ms, callback) {
        const old = this._timeouts.get(kind);
        if (old)
            GLib.source_remove(old);
        const id = GLib.timeout_add(GLib.PRIORITY_DEFAULT, Math.max(0, Math.round(ms)), () => {
            this._timeouts?.delete(kind);
            callback();
            return GLib.SOURCE_REMOVE;
        });
        this._timeouts.set(kind, id);
    }

    // Whether colours are taken from the wallpaper, as amethystora-theme reads it (colours_way in its library): the
    // account's choice, where it made one (off included), else the image's own
    _coloursOn() {
        const way = file => {
            try {
                const [, bytes] = GLib.file_get_contents(file);
                return new TextDecoder().decode(bytes).split('\n')[0].trim();
            } catch {
                return null;
            }
        };
        const own = way(GLib.build_filenamev([CONFIG, 'colours']));
        if (own === 'off')
            return false;
        if (['vivid', 'soft', 'mono'].includes(own))
            return true;
        return ['vivid', 'soft', 'mono'].includes(way(IMAGE_COLOURS));
    }

    _daylightShown() {
        return this._background?.get_string('picture-uri').endsWith('-daylight.xml') ?? false;
    }

    // amethystora-theme with a fixed argument vector, one of each kind at a time: a request while one runs is run once
    // more after it. A failure is said in the journal only.
    _run(kind, args, then = null) {
        const running = this._runs.get(kind);
        if (running) {
            running.again = true;
            running.then = then ?? running.then;
            return;
        }
        const run = {cancellable: new Gio.Cancellable(), again: false, then};
        let process;
        try {
            const launcher = new Gio.SubprocessLauncher({
                flags: Gio.SubprocessFlags.STDOUT_SILENCE | Gio.SubprocessFlags.STDERR_SILENCE,
            });
            // While a focus profile has an accent, every rendering of the accents' rules carries it too
            if (this._focusAccent)
                launcher.setenv('AMETHYSTORA_FOCUS_ACCENT', this._focusAccent, true);
            process = launcher.spawnv([THEME_COMMAND, ...args]);
        } catch (e) {
            console.warn(`Amethystora UI: amethystora-theme ${args.join(' ')} did not start: ${e.message}`);
            return;
        }
        this._runs.set(kind, run);
        process.wait_async(run.cancellable, (proc, result) => {
            try {
                proc.wait_finish(result);
            } catch {
                // Cancelled: the module stopped
                return;
            }
            if (!proc.get_successful())
                console.warn(`Amethystora UI: amethystora-theme ${args.join(' ')} ended with ${proc.get_exit_status()}`);
            if (!this._runs)
                return;
            this._runs.delete(kind);
            if (run.again)
                this._run(kind, args, run.then);
            else
                run.then?.();
        });
    }

    // ---- The stylesheets ----

    // The shell's theme made again with every stylesheet it had, less those to drop and with those to add, and put in
    // place in one go, as GNOME Shell itself does when its theme changes (Main.loadTheme): loading or unloading a single
    // stylesheet restyles the whole stage, so this is one restyle whatever the number of files. Gives back what it added.
    _replace(drop, add) {
        const context = St.ThemeContext.get_for_stage(global.stage);
        const old = context.get_theme();
        const theme = new St.Theme({
            application_stylesheet: old.application_stylesheet,
            theme_stylesheet: old.theme_stylesheet,
            default_stylesheet: old.default_stylesheet,
        });
        for (const file of old.get_custom_stylesheets()) {
            if (drop.some(each => each.equal(file)))
                continue;
            // Another extension's stylesheet that has gone from the disk since it was loaded is left out, not fatal
            try {
                theme.load_stylesheet(file);
            } catch (e) {
                console.warn(`Amethystora UI: ${file.get_path()} was not loaded again: ${e.message}`);
            }
        }
        const added = [];
        for (const [name, file] of add) {
            try {
                theme.load_stylesheet(file);
                added.push([name, file]);
            } catch (e) {
                console.warn(`Amethystora UI: the theme's ${name} was not loaded: ${e.message}`);
            }
        }
        context.set_theme(theme);
        return added;
    }

    _unload() {
        if (this._loaded?.size)
            this._replace([...this._loaded.values()], []);
        this._loaded?.clear();
    }

    // The theme's stylesheets by name, and, while each workspace has its own accent, each accent's rules
    _wanted() {
        const wanted = new Map();
        let names = [];
        try {
            const children = Gio.File.new_for_path(THEME_DIR).enumerate_children('standard::name',
                Gio.FileQueryInfoFlags.NONE, null);
            names = [...children].map(info => info.get_name());
        } catch {
            // No theme applied yet: the modules' own stylesheets stand alone
            return wanted;
        }
        for (const name of names.filter(n => STYLESHEET.test(n)).sort())
            wanted.set(name, Gio.File.new_for_path(GLib.build_filenamev([THEME_DIR, name])));
        if (this._on() || this._focusAccent) {
            for (const id of IDS) {
                const file = Gio.File.new_for_path(GLib.build_filenamev([ACCENTS_DIR, id, 'shell-accent.css']));
                if (file.query_exists(null))
                    wanted.set(`accent-${id}`, file);
            }
        }
        return wanted;
    }

    // Everything again, after a theme switch, a new set of accents or the feature turned on or off: one restyle
    // Before a theme is applied, the modules' stand-in colours are for a dark bar: this class has their dark ones
    // take over on the light scheme. Only then, so that a theme's rules never have to outrank them
    _markUnthemed() {
        if (!GLib.file_test(THEME_DIR, GLib.FileTest.IS_DIR) && Main.getStyleVariant() === 'light')
            Main.uiGroup.add_style_class_name(UNTHEMED_LIGHT);
        else
            Main.uiGroup.remove_style_class_name(UNTHEMED_LIGHT);
    }

    _load() {
        this._markUnthemed();
        const stale = [...this._loaded.values()];
        this._loaded.clear();
        for (const [name, file] of this._replace(stale, [...this._wanted()]))
            this._loaded.set(name, file);
        this._apply();
        this._writeState(this._accent);
        this._prewarm();
        // A focus profile's accent not rendered for this theme yet is asked for once
        const focus = this._focusAccent;
        if (focus && !this._loaded.has(`accent-${focus}`) && !this._focusAsked) {
            this._focusAsked = true;
            this._run('accents', ['accents'], () => this._load());
        }
    }

    // The first time a class is put on the top group, the shell works out the style of everything under it, which takes
    // a few frames; after that, coming back to it is a few milliseconds. So each accent's class is put on and taken off
    // once, one accent per idle moment, after its rules are loaded (at login, a theme switch or a new set): between the two
    // nothing is drawn, so nothing shows, and the first switch to each workspace is as quick as the others.
    _prewarm() {
        if (this._prewarmId)
            GLib.source_remove(this._prewarmId);
        this._prewarmId = 0;
        const ids = [...this._loaded.keys()].filter(n => n.startsWith('accent-')).map(n => n.slice(7));
        if (!ids.length)
            return;
        this._prewarmId = GLib.idle_add(GLib.PRIORITY_LOW, () => {
            const id = ids.shift();
            if (id && id !== this._accent && this._loaded?.has(`accent-${id}`)) {
                Main.uiGroup.add_style_class_name(`amethystora-accent-${id}`);
                if (this._accent)
                    Main.uiGroup.remove_style_class_name(`amethystora-accent-${this._accent}`);
                Main.uiGroup.remove_style_class_name(`amethystora-accent-${id}`);
                if (this._accent)
                    Main.uiGroup.add_style_class_name(`amethystora-accent-${this._accent}`);
            }
            if (ids.length)
                return GLib.SOURCE_CONTINUE;
            this._prewarmId = 0;
            return GLib.SOURCE_REMOVE;
        });
    }

    // ---- An accent per workspace ----

    _on() {
        return this._settings?.get_boolean('accent-per-workspace') ?? false;
    }

    // The shown workspace's accent, while each workspace has its own and its rules are loaded
    _shownAccent() {
        // A focus profile's accent is over every workspace's
        if (this._focusAccent && this._loaded.has(`accent-${this._focusAccent}`))
            return this._focusAccent;
        if (!this._on())
            return null;
        const id = workspaceAccent(this._settings, global.workspace_manager.get_active_workspace_index());
        return id && this._loaded.has(`accent-${id}`) ? id : null;
    }

    _perWorkspaceChanged() {
        if (this._on() && !this._workspaceHandler) {
            global.workspace_manager.connectObject('active-workspace-changed', () => this._workspaceChanged(), this);
            this._workspaceHandler = true;
        } else if (!this._on() && this._workspaceHandler) {
            global.workspace_manager.disconnectObject(this);
            this._workspaceHandler = false;
            const pending = this._timeouts.get('workspace');
            if (pending)
                GLib.source_remove(pending);
            this._timeouts.delete('workspace');
        }
        // The accents' rules are loaded or taken away with the theme's, in one go
        const loaded = [...this._loaded.keys()].some(name => name.startsWith('accent-'));
        if (loaded !== (this._on() || !!this._focusAccent))
            this._load();
        this._accentsChanged();
    }

    // A workspace's accent whose rules are not rendered yet (the menu chose a new one) is asked of amethystora-theme
    // first, and loaded once they are
    _accentsChanged() {
        const missing = this._on() && this._settings.get_strv('workspace-accents').some(id =>
            IDS.includes(id) && !this._loaded.has(`accent-${id}`));
        if (missing)
            this._run('accents', ['accents'], () => this._load());
        this._apply();
    }

    _workspaceChanged() {
        if (!St.Settings.get().enable_animations) {
            this._apply();
            return;
        }
        this._later('workspace', SLIDE * St.Settings.get().slow_down_factor, () => this._apply());
    }

    // The shown workspace's accent put on the shell's top group: a class, which restyles what is under it without
    // reading a stylesheet again
    _apply() {
        const accent = this._shownAccent();
        if (accent === this._accent)
            return;
        const start = GLib.get_monotonic_time();
        if (this._accent)
            Main.uiGroup.remove_style_class_name(`amethystora-accent-${this._accent}`);
        if (accent)
            Main.uiGroup.add_style_class_name(`amethystora-accent-${accent}`);
        this.lastSwapMs = (GLib.get_monotonic_time() - start) / 1000;
        // How many restyles a session has had, for the lab: a run of switches makes one
        this.swaps = (this.swaps ?? 0) + 1;
        this._accent = accent;
        this._writeState(accent);
        if (GLib.getenv('AMETHYSTORA_UI_DEBUG'))
            console.log(`Amethystora UI: the accent ${accent ?? 'theme'} took ${this.lastSwapMs} ms`);
    }

    // What the Amethystora apps read: the shown workspace's accent, or no file for the theme's own. Written whole, with a
    // copy beside it; a file that cannot be written leaves the apps on the theme's colours.
    _writeState(accent) {
        try {
            if (accent) {
                GLib.mkdir_with_parents(STATE_DIR, 0o700);
                GLib.file_set_contents(STATE, `${accent}\n`);
                GLib.file_set_contents(`${STATE}.bak`, `${accent}\n`);
            } else {
                for (const path of [STATE, `${STATE}.bak`]) {
                    if (GLib.file_test(path, GLib.FileTest.EXISTS))
                        GLib.unlink(path);
                }
            }
        } catch (e) {
            console.warn(`Amethystora UI: ${STATE} could not be written: ${e.message}`);
        }
    }
}
