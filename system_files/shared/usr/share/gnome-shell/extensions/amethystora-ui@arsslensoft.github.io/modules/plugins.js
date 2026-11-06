// Plugins in the shell (plans/PLAN-plugins.md, phases 2 and 3; R7-01, 03, 07, 09). A plugin's code never runs here: the plugin host runs
// it in a sandbox and tells this module what to show, and this module draws it from a fixed set of elements, clamped again (plugins/
// clamp.js): the pill's `extra` place, the top bar, a card on the desktop (deskwidgets.js asks this module for them), a tile of quick
// settings, an answer in search and a badge on a dock icon. A script's pill (`ame pill`) is a file this module watches (plugins/script.js).
//
// Off costs nothing: with no plugin on there is no host and nothing but a watch for its bus name; with the key off (`ame desktop plugins
// off`) not even that. Nothing polls. A screen lock disables the extension and the unlock asks the host again, which holds every state,
// so nothing here is kept at module scope. A plugin never gets a system plate (call, recording, link, battery, agent): its activity's
// plate is '', and the activity comes after everything the system shows (pill/activities.js ORDER).

import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import St from 'gi://St';

import {gettext as _} from 'resource:///org/gnome/shell/extensions/extension.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import * as Util from 'resource:///org/gnome/shell/misc/util.js';

import {BarButton} from './plugins/bar.js';
import {plugin as clampPlugin} from './plugins/clamp.js';
import {DockBadges} from './plugins/dock.js';
import {HostClient} from './plugins/host.js';
import {ExtraMark} from './plugins/pill.js';
import {Scripts} from './plugins/script.js';
import {PluginTile} from './plugins/tile.js';
import {shownAddress} from './pill/text.js';

const SCHEMA = 'org.gnome.shell.extensions.amethystora-ui.plugins';
const THEME_DIR = GLib.build_filenamev([GLib.get_user_config_dir(), 'amethystora', 'current']);
const PLUGINS = '/usr/libexec/amethystora-plugins';
const PILL = '/usr/libexec/amethystora-pill';
const IN_TERMINAL = '/usr/libexec/amethystora-in-terminal';
const EMPTY_OUTPUT = {text: '', icon: '', class: '', click: null, hidden: false, private: false, progress: null, until: null, menu: [], card: [], lines: []};

function spawn(argv) {
    try {
        Util.spawn(argv);
    } catch (e) {
        console.warn(`Amethystora UI: ${argv[0]} did not start: ${e.message}`);
    }
}

export class Plugins {
    constructor(ext) {
        this._ext = ext;
    }

    enable() {
        this._settings = this._ext.getSettings(SCHEMA);
        this._settings.connectObject('changed::show', () => this._sync(), this);
        this._listeners = new Set();
        this._sync();
    }

    disable() {
        this._settings?.disconnectObject(this);
        this._stop();
        this._listeners = null;
        // Disposed now rather than by the collector (dock.js says why)
        this._settings?.run_dispose();
        this._settings = null;
    }

    get on() {
        return this._settings?.get_boolean('show') ?? false;
    }

    _sync() {
        if (this.on && !this._host)
            this._start();
        else if (!this.on && this._host)
            this._stop();
    }

    _start() {
        this._palette = {};
        this._plugins = new Map();
        this._seq = 0;
        this._order = new Map();
        this._bars = new Map();
        this._tiles = new Map();
        this._extra = null;
        this._ui = {
            hidden: item => this._hidden(item),
            hiddenOutput: output => this.hiddenOutput(output),
            act: (item, action) => this._act(item, action),
            pause: item => this._host?.pause(item.key),
            resume: item => this._host?.resume(item.key),
            keep: item => spawn([IN_TERMINAL, PLUGINS, 'keep', item.key]),
            change: item => spawn([PLUGINS, 'change', item.key]),
            discard: item => spawn([PLUGINS, 'discard', item.key]),
            stopScript: item => item.file && spawn([PILL, 'end-file', item.file]),
            changed: () => this._pill?.extraChanged?.(),
            pausedWords: p => this._pausedWords(p),
        };
        const self = this;
        Object.defineProperty(this._ui, 'palette', {get: () => self._palette});
        this._host = new HostClient({
            list: list => this._list(list),
            updated: (id, plugin) => this._updated(id, plugin),
            paused: (id, reason) => this._paused(id, reason),
            asked: (id, kind, text) => this._asked(id, kind, text),
            over: (id, host) => this._over(id, host),
            gone: () => this._gone(),
        });
        this._host.start();
        this._scripts = new Scripts(() => this._render(), name => this._said(_('%s updated too often, so its pill ended').format(name)));
        this._scripts.start();
        this._cancellable = new Gio.Cancellable();
    }

    _stop() {
        this._host?.stop();
        this._host = null;
        this._scripts?.stop();
        this._scripts = null;
        this._cancellable?.cancel();
        this._cancellable = null;
        this._themeMonitor?.cancel();
        this._themeMonitor = null;
        this._unlistenCapture?.();
        this._unlistenCapture = null;
        this._clearSurfaces();
        this._plugins = null;
        this._ui = null;
    }

    _clearSurfaces() {
        this._setExtra(null);
        for (const bar of this._bars?.values() ?? [])
            bar.destroy();
        this._bars?.clear();
        for (const tile of this._tiles?.values() ?? [])
            tile.destroy();
        this._tiles?.clear();
        this._badges?.clear();
        this._badges = null;
        this._tellListeners();
    }

    // ---- The host's word ----

    _list(list) {
        this._plugins = new Map();
        for (const raw of list) {
            const plugin = clampPlugin(raw);
            if (plugin)
                this._plugins.set(plugin.id, plugin);
        }
        this._render();
    }

    _updated(id, raw) {
        const plugin = clampPlugin(raw);
        if (!plugin || !this._plugins)
            return;
        // The pill goes to the plugin whose words changed last: one that prints the same again does not take it
        const changed = JSON.stringify(this._plugins.get(plugin.id)?.output ?? null) !== JSON.stringify(plugin.output);
        this._plugins.set(plugin.id, plugin);
        if (changed || !this._order.has(plugin.id))
            this._order.set(plugin.id, ++this._seq);
        this._render();
    }

    _gone() {
        // The host ended (the last plugin was turned off, or it stopped): nothing of theirs is shown
        this._plugins = new Map();
        this._render();
    }

    _paused(id, reason) {
        const plugin = this._plugins?.get(id);
        const name = plugin?.name ?? id;
        const agent = this._ext.modules?.get('agent')?.on === true;
        this._pill?.notice?.(_('%s was paused').format(name), 'media-playback-pause-symbolic', 8, '', null,
            agent ? {label: _('Ask the agent to fix it'), run: () => spawn([PLUGINS, 'fix', id])} : null);
        console.log(`Amethystora UI: plugin ${id} paused itself: ${reason}`);
    }

    _over(id, host) {
        const name = this._plugins?.get(id)?.name ?? id;
        this._said(_('%s sent a lot to %s, so it is blocked until you decide').format(name, host));
    }

    // What a plugin asked the shell to do after a click on its own element: the address is said before it opens
    _asked(id, kind, text) {
        const plugin = this._plugins?.get(id);
        if (!plugin)
            return;
        if (kind === 'open') {
            // The host only passes a web address; the shell, which opens it, checks again
            const address = /^https?:\/\//.test(text) ? shownAddress(text) : null;
            if (!address)
                return;
            this._said(_('%s opened %s').format(plugin.name, address));
            try {
                Gio.AppInfo.launch_default_for_uri(text, global.create_app_launch_context(0, -1));
            } catch (e) {
                console.warn(`Amethystora UI: an address a plugin asked for did not open: ${e.message}`);
            }
        } else if (kind === 'clipboard') {
            St.Clipboard.get_default().set_text(St.ClipboardType.CLIPBOARD, text);
            this._said(_('%s copied something').format(plugin.name));
        }
    }

    _said(text) {
        this._pill?.notice?.(text, 'dialog-information-symbolic', 6);
    }

    _pausedWords(plugin) {
        if (plugin.waiting === 'vault')
            return _('Waiting: its secret settings are in the Vault, which is closed');
        switch (plugin.reason) {
        case 'crash': return _('Paused: it failed several times in a row');
        case 'slow': return _('Paused: it kept taking too long');
        case 'memory': return _('Paused: it kept going over its memory');
        case 'updates': return _('Paused: it updated too often');
        }
        return _('Paused');
    }

    // ---- The surfaces ----

    get _pill() {
        return this._ext.modules?.get('pill') ?? null;
    }

    _output(plugin) {
        // A draft that reaches nothing shows what its author said it would
        return plugin.output && (!plugin.error || !plugin.sample) ? plugin.output : plugin.sample ?? plugin.output;
    }

    _hidden(item) {
        return this.hiddenOutput(item.output);
    }

    // Whether an output's words are private and the screen is being shared, so that no surface shows them
    hiddenOutput(output) {
        return Boolean(output?.private) && Boolean(this._pill?.capture?.active);
    }

    _act(item, action) {
        this._host?.action(item.key, action);
    }

    _render() {
        if (!this._plugins)
            return;
        const plugins = [...this._plugins.values()];
        // Private words hide on every surface while the screen is shared, so the capture is watched while any of them is private
        this._watchCapture(plugins.some(p => this._output(p)?.private) || (this._scripts?.pills ?? []).some(s => s.private));
        this._renderPill(plugins);
        this._renderBars(plugins.filter(p => p.places.bar));
        this._renderTiles(plugins.filter(p => p.places.tile));
        this._renderDock(plugins.filter(p => p.places.dock));
        this._tellListeners();
    }

    // Plugins that want the pill, and scripts: the latest change first
    _renderPill(plugins) {
        const items = [];
        for (const p of plugins) {
            const output = this._output(p);
            if (!p.places.pill)
                continue;
            // One that paused itself stays in the pill, saying so, with Resume in its card: it is not silently gone. One a person paused does not
            const stopped = (p.paused && p.reason !== 'person') || p.waiting !== '';
            if (stopped) {
                items.push({key: p.id, kind: 'plugin', name: p.name, plugin: p, at: this._order.get(p.id) ?? 0,
                    output: {...EMPTY_OUTPUT, text: this._pausedWords(p), icon: 'media-playback-pause-symbolic', class: 'warning'}});
                continue;
            }
            if (!output || output.hidden || p.paused)
                continue;
            items.push({key: p.id, kind: p.draft ? 'draft' : 'plugin', name: p.name, output, plugin: p, at: this._order.get(p.id) ?? 0});
        }
        for (const s of this._scripts?.pills ?? []) {
            const at = this._scriptAt(s);
            items.push({
                key: s.name, file: s.file, kind: 'script', name: s.name, at,
                output: {text: s.text, icon: s.icon, class: '', click: null, hidden: false, private: s.private, progress: s.progress, until: s.until, menu: [], card: []},
                plugin: null,
            });
        }
        items.sort((a, b) => b.at - a.at);
        if (items.length === 0) {
            this._setExtra(null);
            return;
        }
        if (this._extra) {
            this._extra.set(items);
            this._pill?.extraChanged?.();
        } else {
            this._setExtra(new ExtraMark(items, this._ui));
        }
    }

    // Scripts have no host to count their changes: when this module first saw each state is its order
    _scriptAt(script) {
        this._scriptSeen ??= new Map();
        const key = `${script.name}|${script.text}|${script.progress}|${script.until}`;
        const seen = this._scriptSeen.get(script.name);
        if (!seen || seen.key !== key)
            this._scriptSeen.set(script.name, {key, at: ++this._seq});
        return this._scriptSeen.get(script.name).at;
    }

    _setExtra(mark) {
        const pill = this._pill;
        if (this._extra && this._extra !== mark) {
            pill?.setExtra?.(null);
            this._extra.destroy();
        }
        this._extra = mark;
        if (mark)
            pill?.setExtra?.(mark);
    }

    // A private output hides while the screen is shared, which only the capture's own word can say
    _watchCapture(wanted) {
        if (wanted && !this._unlistenCapture)
            this._unlistenCapture = this._pill?.capture?.listen(() => this._render()) ?? null;
        else if (!wanted && this._unlistenCapture) {
            this._unlistenCapture();
            this._unlistenCapture = null;
        }
    }

    _renderBars(plugins) {
        const wanted = new Set(plugins.map(p => p.id));
        for (const [id, bar] of [...this._bars]) {
            if (!wanted.has(id)) {
                bar.destroy();
                this._bars.delete(id);
            }
        }
        if (plugins.length && !this._themeMonitor) {
            this._watchTheme();
        } else if (!plugins.length && this._themeMonitor) {
            this._themeMonitor.cancel();
            this._themeMonitor = null;
        }
        let added = false;
        for (const plugin of plugins) {
            const bar = this._bars.get(plugin.id);
            if (bar) {
                bar.set(plugin);
                continue;
            }
            const button = new BarButton(plugin, this._ui);
            this._bars.set(plugin.id, button);
            Main.panel.addToStatusArea(`amethystora-plugin-${plugin.id}`, button, 0, button.side);
            added = true;
        }
        if (added)
            this._arrange();
    }

    // In "order", then by id, at the inner edge of their part of the bar: after what is on the left and in the centre already, and
    // before what is on the right, so the system menu stays last
    _arrange() {
        const buttons = [...this._bars.entries()].sort(([a, x], [b, y]) => x.rank - y.rank || a.localeCompare(b)).map(([, button]) => button);
        for (const button of buttons.filter(b => b.side !== 'right')) {
            const box = button.container.get_parent();
            box?.set_child_at_index(button.container, box.get_n_children() - 1);
        }
        for (const button of buttons.filter(b => b.side === 'right').reverse())
            button.container.get_parent()?.set_child_at_index(button.container, 0);
    }

    _renderTiles(plugins) {
        const wanted = new Set(plugins.map(p => p.id));
        for (const [id, tile] of [...this._tiles]) {
            if (!wanted.has(id)) {
                tile.destroy();
                this._tiles.delete(id);
            }
        }
        for (const plugin of plugins) {
            const tile = this._tiles.get(plugin.id);
            if (tile) {
                tile.set(plugin);
                continue;
            }
            const made = new PluginTile(plugin, this._ui);
            this._tiles.set(plugin.id, made);
            Main.panel.statusArea.quickSettings?.menu.addItem(made.tile, 1);
        }
    }

    _renderDock(plugins) {
        const launcher = this._ext.modules?.get('dock')?.launcher ?? null;
        if (!launcher) {
            this._badges = null;
            return;
        }
        if (!this._badges || this._badges._launcher !== launcher)
            this._badges = new DockBadges(launcher);
        this._badges.sync(plugins);
    }

    // The theme's palette colours a class in the bar, read again when the theme switches, while a bar plugin is there
    _watchTheme() {
        const read = () => {
            const file = Gio.File.new_for_path(GLib.build_filenamev([THEME_DIR, 'theme', 'colors.toml']));
            file.load_contents_async(this._cancellable, (f, result) => {
                try {
                    const [, bytes] = f.load_contents_finish(result);
                    const text = new TextDecoder().decode(bytes);
                    this._palette = Object.fromEntries([...text.matchAll(/^(\w+)\s*=\s*"(#[0-9a-fA-F]{3,8})"/gm)].map(m => [m[1], m[2]]));
                    this._bars?.forEach(bar => bar.set(bar.plugin));
                } catch {}
            });
        };
        try {
            // The palette is current/theme/colors.toml: the folder it is in, and the one that is swapped
            this._themeMonitor = Gio.File.new_for_path(GLib.build_filenamev([THEME_DIR, 'theme'])).monitor_directory(Gio.FileMonitorFlags.NONE, null);
            this._themeMonitor.connect('changed', () => read());
        } catch {}
        read();
    }

    // ---- What other modules ask ----

    onChanged(callback) {
        this._listeners?.add(callback);
        return () => this._listeners?.delete(callback);
    }

    _tellListeners() {
        for (const callback of [...this._listeners ?? []])
            callback();
    }

    // The plugins that want a corner of the desktop, for modules/deskwidgets.js
    desktopPlugins() {
        return [...this._plugins?.values() ?? []].filter(p => p.places.desktop);
    }

    act(id, action) {
        this._host?.action(id, action);
    }

    resume(id) {
        this._host?.resume(id);
    }

    pause(id) {
        this._host?.pause(id);
    }

    hidden(output) {
        return this.hiddenOutput(output);
    }

    pausedWords(plugin) {
        return this._pausedWords(plugin);
    }

    get palette() {
        return this._palette ?? {};
    }

    // Search: what each plugin that contributes search answers to `text` (the text never leaves the shell)
    searchAnswers(text) {
        const query = text.trim().toLowerCase();
        if (!query)
            return [];
        const found = [];
        for (const plugin of this._plugins?.values() ?? []) {
            const place = plugin.places.search;
            const output = this._output(plugin);
            if (!place || !output || plugin.paused)
                continue;
            const prefix = place.prefix.toLowerCase();
            const match = prefix ? query.startsWith(prefix) : plugin.name.toLowerCase().startsWith(query.split(/\s+/)[0]);
            if (match)
                found.push(this._answer(plugin, output));
        }
        return found;
    }

    answerOf(id) {
        const plugin = this._plugins?.get(id);
        const output = plugin && this._output(plugin);
        return output ? this._answer(plugin, output) : null;
    }

    _answer(plugin, output) {
        const action = output.click ?? output.card.find(e => e.action)?.action ?? null;
        return {
            id: plugin.id, name: plugin.name, text: this.hiddenOutput(output) ? '' : output.text, icon: output.icon, draft: plugin.draft, action,
            run: action ? () => this.act(plugin.id, action) : null,
        };
    }

    // For the suites and the headless harness: what is shown now
    snapshot() {
        return {
            plugins: [...this._plugins?.keys() ?? []],
            pill: this._extra?.text ?? null,
            bars: [...this._bars?.keys() ?? []],
            tiles: [...this._tiles?.keys() ?? []],
            scripts: (this._scripts?.pills ?? []).map(s => s.name),
        };
    }
}
