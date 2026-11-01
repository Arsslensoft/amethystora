// The theme block of quick settings: every installed theme as a swatch of its ground, its accent and its text, and a tap
// that switches to it with `ame desktop theme`, the same switch as Control's. The shell and Amethystora's apps take the
// new colours at once, other apps at their next start.
//
// Nothing is read until quick settings opens: then the two folders of themes are listed and each colors.toml read, and
// the current theme is followed for as long as it stays open. A tap runs only `ame desktop theme NAME`, as an argument
// vector, with the name of a folder that was listed here and holds a colors.toml; no other name is ever passed.

import Atk from 'gi://Atk';
import Clutter from 'gi://Clutter';
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import Pango from 'gi://Pango';
import St from 'gi://St';

import {gettext as _} from 'resource:///org/gnome/shell/extensions/extension.js';

Gio._promisify(Gio.File.prototype, 'enumerate_children_async');
Gio._promisify(Gio.FileEnumerator.prototype, 'next_files_async');
Gio._promisify(Gio.File.prototype, 'load_contents_async', 'load_contents_finish');

const AME = '/usr/bin/ame';
const SYSTEM_THEMES = '/usr/share/amethystora/themes';
const CURRENT = GLib.build_filenamev([GLib.get_user_config_dir(), 'amethystora', 'current']);
// A theme's folder name, as theme_slug in /usr/lib/amethystora/theme/lib.sh makes it
const SLUG = /^[a-z0-9][a-z0-9._-]{0,63}$/;
const COLOUR = /^\s*(background|foreground|accent|color4)\s*=\s*["']?(#[0-9a-fA-F]{6})["']?\s*$/;
// The swatches' grid: columns of 62 pixels, as many as fit in the block (378 less its padding of 14 on each side)
const COLUMNS = 5;
const NAME_WIDTH = 66;

function userThemes() {
    return GLib.build_filenamev([GLib.get_user_config_dir(), 'amethystora', 'themes']);
}

// "tokyo-night" as "Tokyo Night", as Control writes a theme's name
export function prettyName(slug) {
    return slug.split(/[-_]+/).filter(Boolean).map(word => word[0].toUpperCase() + word.slice(1)).join(' ');
}

function parseColours(text) {
    const colours = {};
    for (const line of text.split('\n')) {
        const match = COLOUR.exec(line);
        if (match && !colours[match[1]])
            colours[match[1]] = match[2];
    }
    colours.accent ??= colours.color4;
    return colours.background && colours.foreground && colours.accent ? colours : null;
}

async function listFolders(path, cancellable) {
    const names = [];
    try {
        const children = await Gio.File.new_for_path(path).enumerate_children_async('standard::name,standard::type',
            Gio.FileQueryInfoFlags.NONE, GLib.PRIORITY_DEFAULT, cancellable);
        for (;;) {
            // eslint-disable-next-line no-await-in-loop
            const infos = await children.next_files_async(32, GLib.PRIORITY_DEFAULT, cancellable);
            if (infos.length === 0)
                break;
            for (const info of infos) {
                if (info.get_file_type() === Gio.FileType.DIRECTORY && SLUG.test(info.get_name()))
                    names.push(info.get_name());
            }
        }
    } catch (e) {
        if (e.matches?.(Gio.IOErrorEnum, Gio.IOErrorEnum.CANCELLED))
            throw e;
    }
    return names;
}

async function readText(path, cancellable) {
    const [bytes] = await Gio.File.new_for_path(path).load_contents_async(cancellable);
    return new TextDecoder().decode(bytes);
}

export class ThemeBlock {
    constructor() {
        this._themes = [];
        this._swatches = new Map();
        this.block = new St.BoxLayout({
            style_class: 'amethystora-quick-block amethystora-quick-theme',
            orientation: Clutter.Orientation.VERTICAL,
            x_expand: true,
            visible: false,
        });
        this.block._amethystoraQuickId = 'theme';
        this.block.add_child(new St.Label({style_class: 'amethystora-quick-block-title', text: _('Theme')}));
        this._layout = new Clutter.GridLayout({column_spacing: 6, row_spacing: 8});
        this._grid = new St.Widget({style_class: 'amethystora-quick-swatches', layout_manager: this._layout});
        this.block.add_child(this._grid);
        this._error = new St.Label({style_class: 'amethystora-quick-error', visible: false,
            text: _('The theme could not be applied.')});
        this._error.clutter_text.line_wrap = true;
        this.block.add_child(this._error);
        this.block.connect('notify::mapped', () => {
            if (this.block.mapped && this._unfitted)
                this._fitNames();
        });
    }

    // Quick settings opened: the themes as they are now, and the current one followed while it stays open
    opened() {
        this._cancellable?.cancel();
        this._cancellable = new Gio.Cancellable();
        this._load(this._cancellable).catch(e => {
            if (!e.matches?.(Gio.IOErrorEnum, Gio.IOErrorEnum.CANCELLED))
                console.warn(`Amethystora UI: the themes were not read: ${e.message}`);
        });
        try {
            this._monitor = Gio.File.new_for_path(CURRENT).monitor_directory(Gio.FileMonitorFlags.WATCH_MOVES, null);
            this._monitor.connect('changed', (monitor, file, other) => {
                if ([file, other].some(each => each?.get_basename() === 'theme.name'))
                    this._readCurrent(this._cancellable);
            });
        } catch {
            // No theme applied yet: nothing to follow
        }
    }

    closed() {
        this._cancellable?.cancel();
        this._cancellable = null;
        this._monitor?.cancel();
        this._monitor = null;
        this._error.hide();
    }

    async _load(cancellable) {
        const system = await listFolders(SYSTEM_THEMES, cancellable);
        const user = await listFolders(userThemes(), cancellable);
        // Each name once, sorted, the account's own folder over a shipped one of the same name (as Control reads them)
        const slugs = [...new Set([...system, ...user])].sort();
        const themes = [];
        for (const slug of slugs) {
            const folder = user.includes(slug) ? userThemes() : SYSTEM_THEMES;
            let colours = null;
            try {
                // eslint-disable-next-line no-await-in-loop
                colours = parseColours(await readText(GLib.build_filenamev([folder, slug, 'colors.toml']), cancellable));
            } catch (e) {
                if (e.matches?.(Gio.IOErrorEnum, Gio.IOErrorEnum.CANCELLED))
                    throw e;
            }
            if (colours)
                themes.push({slug, colours});
        }
        if (JSON.stringify(themes) !== JSON.stringify(this._themes)) {
            this._themes = themes;
            this._build();
        }
        await this._readCurrent(cancellable);
    }

    async _readCurrent(cancellable) {
        if (!cancellable || cancellable.is_cancelled())
            return;
        try {
            const name = (await readText(GLib.build_filenamev([CURRENT, 'theme.name']), cancellable)).trim();
            if (!this._running)
                this._mark(name);
        } catch {
            if (!this._running)
                this._mark(null);
        }
    }

    _build() {
        this._grid.destroy_all_children();
        this._swatches.clear();
        this._themes.forEach(({slug, colours}, index) => {
            const swatch = this._swatch(slug, colours);
            this._layout.attach(swatch, index % COLUMNS, Math.floor(index / COLUMNS), 1, 1);
            this._swatches.set(slug, swatch);
        });
        this.block.visible = this._themes.length > 0;
        this._fitNames();
    }

    // Each name broken into two lines where its words fit the column; a word too long for it is ellipsized
    _fitNames() {
        // The font is known only on the stage: a block put away is fitted when it is shown again
        this._unfitted = !this.block.get_stage();
        if (this._unfitted)
            return;
        for (const swatch of this._swatches.values()) {
            const [first, second] = swatch._names;
            const words = swatch._words;
            const fits = text => {
                first.text = text;
                return first.clutter_text.get_preferred_width(-1)[1] <= NAME_WIDTH;
            };
            let n = 1;
            while (n < words.length && fits(words.slice(0, n + 1).join(' ')))
                n++;
            first.text = words.slice(0, n).join(' ');
            second.text = words.slice(n).join(' ');
            second.visible = n < words.length;
        }
    }

    _swatch(slug, colours) {
        const name = prettyName(slug);
        const button = new St.Button({
            style_class: 'amethystora-quick-swatch',
            can_focus: true,
            accessible_name: _('Theme %s').format(name),
            accessible_role: Atk.Role.TOGGLE_BUTTON,
            y_align: Clutter.ActorAlign.START,
        });
        const box = new St.BoxLayout({orientation: Clutter.Orientation.VERTICAL, x_align: Clutter.ActorAlign.CENTER});
        // A swatch is colours, not text: its stripes keep their order, and their rounded ends, right to left too
        const square = new St.BoxLayout({style_class: 'amethystora-quick-swatch-square',
            text_direction: Clutter.TextDirection.LTR});
        ['background', 'accent', 'foreground'].forEach((key, i) => {
            square.add_child(new St.Widget({
                style_class: `amethystora-quick-swatch-stripe amethystora-quick-swatch-stripe-${i}`,
                style: `background-color: ${colours[key]};`,
                x_expand: true,
            }));
        });
        // Two rings round the current theme, the text's colour inside the accent's (St has no outline)
        const inner = new St.Bin({style_class: 'amethystora-quick-swatch-inner', child: square});
        const outer = new St.Bin({style_class: 'amethystora-quick-swatch-outer', child: inner,
            x_align: Clutter.ActorAlign.CENTER});
        box.add_child(outer);
        // The name on two lines at most, broken between words, each line ellipsized (fitted once the font is known)
        button._names = [0, 1].map(() => {
            const line = new St.Label({style_class: 'amethystora-quick-swatch-name', x_align: Clutter.ActorAlign.CENTER});
            line.clutter_text.ellipsize = Pango.EllipsizeMode.END;
            box.add_child(line);
            return line;
        });
        button._words = name.split(' ');
        button._names[0].text = name;
        button._names[1].visible = false;
        button.set_child(box);
        button.connect('clicked', () => this.pick(slug));
        return button;
    }

    _mark(current) {
        this._current = current;
        for (const [slug, swatch] of this._swatches) {
            if (slug === current) {
                swatch.add_style_pseudo_class('checked');
                swatch.add_accessible_state(Atk.StateType.CHECKED);
            } else {
                swatch.remove_style_pseudo_class('checked');
                swatch.remove_accessible_state(Atk.StateType.CHECKED);
            }
        }
    }

    // A tap: the theme named by a folder listed here, by `ame desktop theme`; false for any other name
    pick(slug) {
        if (typeof slug !== 'string' || !SLUG.test(slug) || !this._swatches.has(slug) || this._running)
            return false;
        if (slug === this._current)
            return true;
        const previous = this._current;
        this._mark(slug);
        this._error.hide();
        this._running = true;
        for (const swatch of this._swatches.values())
            swatch.reactive = false;
        let proc;
        try {
            proc = Gio.Subprocess.new([AME, 'desktop', 'theme', slug],
                Gio.SubprocessFlags.STDOUT_SILENCE | Gio.SubprocessFlags.STDERR_SILENCE);
        } catch (e) {
            console.warn(`Amethystora UI: the theme could not be switched: ${e.message}`);
            this._done(false, previous);
            return true;
        }
        proc.wait_check_async(null, (p, result) => {
            let ok = false;
            try {
                ok = p.wait_check_finish(result);
            } catch {}
            this._done(ok, previous);
        });
        return true;
    }

    _done(ok, previous) {
        this._running = false;
        if (!this._swatches)
            return;
        for (const swatch of this._swatches.values())
            swatch.reactive = true;
        if (!ok) {
            this._mark(previous);
            if (this._cancellable)
                this._error.show();
        } else if (this._cancellable) {
            this._readCurrent(this._cancellable);
        }
    }

    // For the tests: the swatches and which one is marked
    swatches() {
        return [...this._swatches.keys()].map(slug => ({slug, marked: slug === this._current}));
    }

    destroy() {
        this.closed();
        this._swatches.clear();
        this._swatches = null;
        this.block.destroy();
    }
}
