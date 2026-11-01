// The window switcher of the classic layout: Super+Tab and Alt+Tab show every window as a live picture in a grid on
// glass, in the order they were last used. Tab, the arrows and the pointer move the choice, and letting go of the key
// switches, as GNOME's switcher does. Typing filters: only the windows whose app or title has every word typed stay,
// and from the first letter on the grid stays open after the key is let go, until Enter switches or Escape leaves
// everything as it was. The pictures are clones of the windows, never copies, and go when the grid closes.

import Clutter from 'gi://Clutter';
import GLib from 'gi://GLib';
import Gio from 'gi://Gio';
import Meta from 'gi://Meta';
import Pango from 'gi://Pango';
import Shell from 'gi://Shell';
import St from 'gi://St';

import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import {gettext as _, pgettext as C_} from 'resource:///org/gnome/shell/extensions/extension.js';

import {fold, label} from './util.js';

const NAMES = ['switch-applications', 'switch-applications-backward', 'switch-windows', 'switch-windows-backward'];
// GNOME's own: a quick press and release switches without drawing anything
const POPUP_DELAY = 150;
const FADE_IN = 150;
const FADE_OUT = 100;
// The board's measures, in the stylesheet's pixels
const THUMB = [178, 110];
const GAP = 12;
const ICON = 24;
const OVERHANG = 6;
const MAX_COLUMNS = 6;

const scale = () => St.ThemeContext.get_for_stage(global.stage).scale_factor;

// The modifier that holds the switcher open: the highest one of the binding's
function primary(mask) {
    let bit = 1;
    while (mask > 1) {
        mask >>= 1;
        bit <<= 1;
    }
    return mask ? bit : 0;
}

// The windows to show, most recently used first, each once: a dialog fixed to its window is reached through that
// window, and a window that keeps itself out of the taskbar is left out, as the person expects of a switcher
function listed(workspace) {
    const seen = new Set();
    for (let window of global.display.get_tab_list(Meta.TabList.NORMAL_ALL, workspace)) {
        while (window?.is_attached_dialog())
            window = window.get_transient_for();
        if (window && !window.skip_taskbar)
            seen.add(window);
    }
    return [...seen];
}

// A text folded for matching, with the place in the original of every folded character, so that a match found in
// the folded text can be shown on the original
function foldWithMap(text) {
    let folded = '';
    const map = [];
    let at = 0;
    for (const char of text) {
        const piece = fold(char);
        for (let i = 0; i < piece.length; i++)
            map.push(at);
        folded += piece;
        at += char.length;
    }
    map.push(at);
    return {folded, map};
}

// The byte offset, in UTF-8, of a place in a JavaScript string
const bytes = (text, index) => new TextEncoder().encode(text.slice(0, index)).length;

export class Switcher {
    constructor(module) {
        this._module = module;
        this._ext = module.ext;
    }

    enable() {
        this._appSettings = new Gio.Settings({schema_id: 'org.gnome.shell.app-switcher'});
        this._windowSettings = new Gio.Settings({schema_id: 'org.gnome.shell.window-switcher'});
        for (const name of NAMES) {
            Main.wm.setCustomKeybindingHandler(name, Shell.ActionMode.NORMAL,
                (display, window, event, binding) => this.open(binding.get_name(), binding.is_reversed(),
                    binding.get_mask()));
        }
    }

    disable() {
        this.close(true);
        this._fading?.destroy();
        this._fading = null;
        this._fadingPane = null;
        // GNOME's own switchers back, unless the tiling has taken one of these keys for itself meanwhile
        const wrapped = this._ext.modules?.get('tiling')?.keys?._wrapped;
        for (const name of NAMES) {
            if (wrapped?.has?.(name))
                continue;
            Main.wm.setCustomKeybindingHandler(name, Shell.ActionMode.NORMAL, Main.wm._startSwitcher.bind(Main.wm));
        }
        // Disposed now rather than by the collector (dock.js says why)
        this._appSettings?.run_dispose();
        this._appSettings = null;
        this._windowSettings?.run_dispose();
        this._windowSettings = null;
    }

    get visible() {
        return !!this._root && this._shown;
    }

    // For tests: the labels of the tiles listed now
    titles() {
        return (this._shownTiles ?? []).map(t => t.text);
    }

    get query() {
        return this._query ?? '';
    }

    get countText() {
        return this._count?.text ?? '';
    }

    glassSurfaces() {
        const pane = this._pane ?? this._fadingPane;
        return pane ? [{actor: pane, material: 'glass', radius: 22}] : [];
    }

    // --- opening --------------------------------------------------------------------------------------------------

    open(name = 'switch-windows', backward = false, mask = 0) {
        if (this._root)
            return false;
        const apps = name.startsWith('switch-applications');
        const settings = apps ? this._appSettings : this._windowSettings;
        const workspace = settings.get_boolean('current-workspace-only')
            ? global.workspace_manager.get_active_workspace() : null;
        const windows = listed(workspace);
        if (!windows.length)
            return false;

        this._windows = windows;
        this._mask = primary(mask);
        this._query = '';
        this._typed = false;
        this._shown = false;
        this._build();

        this._grab = Main.pushModal(this._root, {actionMode: Shell.ActionMode.POPUP});
        this._root.grab_key_focus();
        // While the grid has the keyboard, a typed letter is the query's, never another program's shortcut: the
        // accelerators GNOME hands to its settings daemon (Super+L locks) are allowed in a popup, so they are held
        // back here, after GNOME's own filter, until the grid closes. GNOME's built-in keys it filters itself.
        global.window_manager.connectObject('filter-keybinding', (wm, binding) => !binding.is_builtin(), this);
        this._filter();
        this._select(backward ? this._shownTiles.length - 1 : Math.min(1, this._shownTiles.length - 1));

        // The key may be let go before the grab took hold
        if (this._mask) {
            const [, , mods] = global.get_pointer();
            if (!(mods & this._mask)) {
                this._finish();
                return true;
            }
        }
        this._delayId = GLib.timeout_add(GLib.PRIORITY_DEFAULT, POPUP_DELAY, () => {
            this._delayId = 0;
            this._showNow();
            return GLib.SOURCE_REMOVE;
        });
        return true;
    }

    _showNow() {
        if (this._delayId) {
            GLib.source_remove(this._delayId);
            this._delayId = 0;
        }
        if (!this._root || this._shown)
            return;
        this._shown = true;
        Main.osdWindowManager?.hideAll?.();
        for (const actor of [this._pane, ...this._veils])
            actor.ease({opacity: 255, duration: FADE_IN, mode: Clutter.AnimationMode.EASE_OUT_QUAD});
    }

    _build() {
        const s = scale();
        this._root = new St.Widget({reactive: true, name: 'amethystoraWindowsSwitcher'});
        this._root.add_constraint(new Clutter.BindConstraint({source: global.stage, coordinate: Clutter.BindCoordinate.ALL}));
        Main.uiGroup.add_child(this._root);

        this._veils = Main.layoutManager.monitors.map(m => {
            const veil = new St.Widget({style_class: 'amethystora-windows-veil', opacity: 0,
                x: m.x, y: m.y, width: m.width, height: m.height});
            this._root.add_child(veil);
            return veil;
        });

        const monitor = Main.layoutManager.currentMonitor ?? Main.layoutManager.primaryMonitor;
        this._monitorIndex = monitor.index;
        const area = Main.layoutManager.getWorkAreaForMonitor(monitor.index);

        this._pane = new St.BoxLayout({style_class: 'amethystora-windows-switcher', opacity: 0,
            orientation: Clutter.Orientation.VERTICAL, reactive: true});
        this._root.add_child(this._pane);

        // The search field
        const field = new St.BoxLayout({style_class: 'amethystora-windows-switcher-field'});
        this._icon = new St.Icon({icon_name: 'system-search-symbolic', style_class: 'amethystora-windows-switcher-search'});
        field.add_child(this._icon);
        const typed = new St.BoxLayout({style_class: 'amethystora-windows-switcher-typed', y_align: Clutter.ActorAlign.CENTER});
        this._typedLabel = label('', 'amethystora-windows-switcher-query');
        this._caret = new St.Widget({style_class: 'amethystora-windows-switcher-caret', y_align: Clutter.ActorAlign.CENTER});
        this._hint = label(_('Type to filter'), 'amethystora-windows-switcher-hint');
        typed.add_child(this._typedLabel);
        typed.add_child(this._caret);
        typed.add_child(this._hint);
        field.add_child(typed);
        field.add_child(new St.Widget({x_expand: true}));
        this._count = label('', 'amethystora-windows-switcher-count');
        this._count.y_align = Clutter.ActorAlign.CENTER;
        field.add_child(this._count);
        this._pane.add_child(field);

        // The grid
        const fit = Math.floor((area.width * 0.8 + GAP * s) / ((THUMB[0] + GAP) * s));
        this._columns = Math.max(1, Math.min(MAX_COLUMNS, fit, this._windows.length));
        this._grid = new St.BoxLayout({style_class: 'amethystora-windows-switcher-grid',
            orientation: Clutter.Orientation.VERTICAL});
        this._scroll = new St.ScrollView({style_class: 'amethystora-windows-switcher-scroll',
            hscrollbar_policy: St.PolicyType.NEVER, vscrollbar_policy: St.PolicyType.AUTOMATIC,
            overlay_scrollbars: true, child: this._grid});
        this._pane.add_child(this._scroll);
        this._empty = label(_('No window matches'), 'amethystora-windows-switcher-empty');
        this._empty.x_align = Clutter.ActorAlign.CENTER;
        this._empty.hide();
        this._pane.add_child(this._empty);

        this._tiles = this._windows.map(window => this._tile(window, s));
        // A window that closes while the grid is open leaves it
        for (const tile of this._tiles)
            tile.window.connectObject('unmanaging', () => this._gone(tile), this);

        // The footer: one sentence, its two key names drawn as caps
        const footer = new St.BoxLayout({style_class: 'amethystora-windows-switcher-footer',
            x_align: Clutter.ActorAlign.CENTER});
        const sentence = _('{enter} switches · {esc} leaves everything as it was');
        for (const part of sentence.split(/(\{enter\}|\{esc\})/)) {
            if (!part)
                continue;
            if (part === '{enter}' || part === '{esc}') {
                footer.add_child(label(part === '{enter}' ? C_('key', 'Enter') : C_('key', 'Esc'),
                    'amethystora-windows-switcher-key'));
            } else {
                footer.add_child(label(part, 'amethystora-windows-switcher-footer-text'));
            }
        }
        this._pane.add_child(footer);

        // Rows that fit in 70 % of the height, the rest scrolled
        const tileHeight = this._tiles[0].actor.get_preferred_height(-1)[1];
        const rowsFit = Math.max(1, Math.floor((area.height * 0.7 + GAP * s) / (tileHeight + GAP * s)));
        const rows = Math.min(rowsFit, Math.ceil(this._windows.length / this._columns));
        this._scroll.height = rows * tileHeight + (rows - 1) * GAP * s;
        // A scroll bar only when there is more than fits
        this._scroll.vscrollbar_policy = rowsFit < Math.ceil(this._windows.length / this._columns)
            ? St.PolicyType.AUTOMATIC : St.PolicyType.EXTERNAL;
        this._scroll.width = this._columns * THUMB[0] * s + (this._columns - 1) * GAP * s;

        // Centred on the current monitor
        const [, width] = this._pane.get_preferred_width(-1);
        const [, height] = this._pane.get_preferred_height(width);
        this._pane.set_position(Math.round(monitor.x + (monitor.width - width) / 2),
            Math.round(monitor.y + (monitor.height - height) / 2));

        this._root.connectObject(
            'key-press-event', (a, event) => this._keyPress(event),
            'key-release-event', (a, event) => this._keyRelease(event),
            'button-press-event', (a, event) => this._buttonPress(event), this);
        Main.layoutManager.connectObject('system-modal-opened', () => this.close(), this);
        global.display.connectObject('window-created', () => this.close(), this);
        this._module.surfacesChanged();
    }

    _tile(window, s) {
        const app = Shell.WindowTracker.get_default().get_window_app(window);
        const title = window.get_title() ?? '';
        const appName = app?.get_name() ?? '';
        const text = appName ? `${appName} · ${title}` : title;
        const actor = new St.BoxLayout({style_class: 'amethystora-windows-switcher-tile',
            orientation: Clutter.Orientation.VERTICAL, reactive: true, width: THUMB[0] * s});
        const holder = new St.Widget({width: THUMB[0] * s, height: THUMB[1] * s});
        const thumb = new St.Widget({style_class: 'amethystora-windows-switcher-thumb',
            width: THUMB[0] * s, height: THUMB[1] * s});
        holder.add_child(thumb);
        // The selection's ring, drawn once and only shown or hidden: changing the thumbnail's style instead made the
        // first step after opening restyle it, which took milliseconds on a machine drawing in software
        // A 3 px border just outside the thumbnail, as the board's ring (a box shadow would fill the inside too)
        const ring = new St.Widget({style_class: 'amethystora-windows-switcher-ring', opacity: 0,
            x: -3 * s, y: -3 * s, width: (THUMB[0] + 6) * s, height: (THUMB[1] + 6) * s});
        holder.add_child(ring);
        const windowActor = window.get_compositor_private();
        if (windowActor) {
            // Only the window's frame, scaled to fit inside the border, centred
            const frame = window.get_frame_rect();
            const buffer = window.get_buffer_rect();
            const inner = [(THUMB[0] - 2) * s, (THUMB[1] - 2) * s];
            const k = Math.min(inner[0] / Math.max(1, frame.width), inner[1] / Math.max(1, frame.height), 1);
            const w = Math.round(frame.width * k);
            const h = Math.round(frame.height * k);
            const clip = new St.Widget({clip_to_allocation: true, width: w, height: h,
                x: Math.round(s + (inner[0] - w) / 2), y: Math.round(s + (inner[1] - h) / 2)});
            const clone = new Clutter.Clone({source: windowActor, scale_x: k, scale_y: k,
                x: -Math.round((frame.x - buffer.x) * k), y: -Math.round((frame.y - buffer.y) * k)});
            clip.add_child(clone);
            thumb.add_child(clip);
        }
        if (app) {
            const icon = app.create_icon_texture(ICON);
            icon.style_class = 'amethystora-windows-switcher-app';
            const rtl = Clutter.get_default_text_direction() === Clutter.TextDirection.RTL;
            icon.set_position(rtl ? -OVERHANG * s : (THUMB[0] - ICON + OVERHANG) * s, (THUMB[1] - ICON + OVERHANG) * s);
            holder.add_child(icon);
        }
        actor.add_child(holder);
        const name = label(text, 'amethystora-windows-switcher-label');
        // St sets the text's attributes again whenever the label is styled: the highlight is put back after it
        name.connect_after('style-changed', () => {
            tile.base = name.clutter_text.get_attributes();
            if (tile.words?.length)
                this._highlight(tile, tile.words);
        });
        name.x_align = Clutter.ActorAlign.FILL;
        name.clutter_text.x_align = Clutter.ActorAlign.CENTER;
        actor.add_child(name);

        const tile = {window, app, actor, thumb, ring, name, text, title, appName,
            appId: (app?.get_id() ?? '').replace(/\.desktop$/, '')};
        actor.connectObject('motion-event', () => {
            const index = this._shownTiles.indexOf(tile);
            if (index >= 0 && index !== this._selected)
                this._select(index);
            return Clutter.EVENT_PROPAGATE;
        }, this);
        const click = new Clutter.ClickGesture();
        click.connect('recognize', () => {
            const index = this._shownTiles.indexOf(tile);
            if (index >= 0) {
                this._select(index);
                this._finish();
            }
        });
        actor.add_action(click);
        return tile;
    }

    // --- the query ------------------------------------------------------------------------------------------------

    // The tiles that match every word typed, in the spec's order; their matched letters highlighted
    _filter() {
        const words = fold(this._query).split(/\s+/).filter(Boolean);
        let shown = this._tiles;
        if (words.length) {
            const groups = [[], [], []];
            for (const tile of this._tiles) {
                const app = fold(tile.appName);
                const title = fold(tile.title);
                const id = fold(tile.appId);
                if (!words.every(w => app.includes(w) || title.includes(w) || id.includes(w)))
                    continue;
                const first = words[0];
                if (app.startsWith(first))
                    groups[0].push(tile);
                else if (title.split(/\s+/).some(part => part.startsWith(first)))
                    groups[1].push(tile);
                else
                    groups[2].push(tile);
            }
            shown = groups.flat();
        }
        this._shownTiles = shown;
        for (const tile of this._tiles)
            this._highlight(tile, words);
        this._layoutGrid();
        this._typedLabel.text = this._query;
        this._hint.visible = !this._query;
        this._caret.visible = !!this._query;
        this._count.text = this._query
            ? _('{shown} of {total}').replace('{shown}', () => `${shown.length}`)
                .replace('{total}', () => `${this._tiles.length}`)
            : `${this._tiles.length}`;
        this._empty.visible = shown.length === 0;
        this._scroll.visible = shown.length > 0;
        for (const tile of this._tiles)
            tile.ring.opacity = 0;
        this._selected = -1;
        if (shown.length)
            this._select(0);
    }

    _layoutGrid() {
        for (const row of this._grid.get_children()) {
            for (const child of row.get_children())
                row.remove_child(child);
            row.destroy();
        }
        let row = null;
        this._shownTiles.forEach((tile, i) => {
            if (i % this._columns === 0) {
                row = new St.BoxLayout({style_class: 'amethystora-windows-switcher-row'});
                this._grid.add_child(row);
            }
            row.add_child(tile.actor);
        });
    }

    // The letters of the label that match a word, as attributes of the text: never as markup
    _highlight(tile, words) {
        const text = tile.name.clutter_text;
        tile.words = words;
        if (!words.length) {
            text.set_attributes(tile.base ?? null);
            return;
        }
        const colour = this._icon.get_theme_node?.()?.get_foreground_color?.();
        // On top of what St set for the label's style
        const list = tile.base?.copy() ?? new Pango.AttrList();
        const {folded, map} = foldWithMap(tile.text);
        for (const word of words) {
            let from = 0;
            for (;;) {
                const at = folded.indexOf(word, from);
                if (at < 0)
                    break;
                const start = bytes(tile.text, map[at]);
                const end = bytes(tile.text, map[at + word.length]);
                const background = colour
                    ? Pango.attr_background_new(colour.red * 257, colour.green * 257, colour.blue * 257)
                    : Pango.attr_background_new(0x9100, 0x4800, 0xf000);
                const alpha = Pango.attr_background_alpha_new(Math.round(0.35 * 65535));
                const foreground = Pango.attr_foreground_new(65535, 65535, 65535);
                for (const attr of [background, alpha, foreground]) {
                    attr.start_index = start;
                    attr.end_index = end;
                    list.insert(attr);
                }
                from = at + word.length;
            }
        }
        text.set_attributes(list);
    }

    // For tests and screenshots: type as a person would
    type(text) {
        for (const char of text)
            this._add(char);
        return this.titles();
    }

    _add(char) {
        this._typed = true;
        this._query += char;
        this._filter();
        this._showNow();
    }

    // --- choosing -------------------------------------------------------------------------------------------------

    _select(index) {
        const t0 = GLib.get_monotonic_time();
        const tiles = this._shownTiles ?? [];
        if (!tiles.length)
            return;
        index = (index + tiles.length) % tiles.length;
        if (tiles[this._selected])
            tiles[this._selected].ring.opacity = 0;
        this._selected = index;
        const tile = tiles[index];
        tile.ring.opacity = 255;
        this._scrollTo(tile.actor);
        this._module.note('switcherSelect', t0);
    }

    _scrollTo(actor) {
        // A grid that fits has nothing to scroll, and asking where a row is would lay the grid out again
        if (this._scroll.vscrollbar_policy === St.PolicyType.EXTERNAL)
            return;
        const adjustment = this._scroll.vadjustment;
        const row = actor.get_parent();
        if (!row || !adjustment)
            return;
        const box = row.get_allocation_box();
        const [value, , , , , page] = adjustment.get_values();
        if (box.y1 < value)
            adjustment.value = box.y1;
        else if (box.y2 > value + page)
            adjustment.value = box.y2 - page;
    }

    _gone(tile) {
        tile.window.disconnectObject(this);
        this._tiles = this._tiles.filter(t => t !== tile);
        if (!this._tiles.length) {
            this.close();
            return;
        }
        const selected = this._shownTiles[this._selected];
        tile.actor.destroy();
        this._filter();
        const index = this._shownTiles.indexOf(selected);
        if (index >= 0)
            this._select(index);
    }

    _finish() {
        const tile = this._shownTiles?.[this._selected];
        this.close();
        // A window closed at that very moment has nothing to switch to
        if (tile?.window.get_compositor_private())
            Main.activateWindow(tile.window);
    }

    // now: without the fade, as the module stops
    close(now = false) {
        if (!this._root)
            return;
        if (this._delayId)
            GLib.source_remove(this._delayId);
        this._delayId = 0;
        if (this._grab)
            Main.popModal(this._grab);
        this._grab = null;
        Main.layoutManager.disconnectObject(this);
        global.display.disconnectObject(this);
        global.window_manager.disconnectObject(this);
        for (const tile of this._tiles ?? [])
            tile.window.disconnectObject(this);
        const root = this._root;
        root.disconnectObject(this);
        this._root = null;
        const pane = this._pane;
        this._pane = null;
        this._shownTiles = [];
        this._tiles = [];
        this._windows = [];
        const finish = () => {
            if (this._fading === root) {
                this._fading = null;
                this._fadingPane = null;
                this._module.surfacesChanged();
            }
            root.destroy();
        };
        // One grid fades at a time: an earlier one still fading goes at once
        if (this._fading && this._fading !== root) {
            this._fading.destroy();
            this._fading = null;
            this._fadingPane = null;
        }
        if (this._shown && !now && St.Settings.get().enable_animations) {
            this._fading = root;
            this._fadingPane = pane;
            this._shown = false;
            root.reactive = false;
            for (const veil of this._veils)
                veil.ease({opacity: 0, duration: FADE_OUT, mode: Clutter.AnimationMode.EASE_OUT_QUAD});
            pane.ease({opacity: 0, duration: FADE_OUT, mode: Clutter.AnimationMode.EASE_OUT_QUAD, onComplete: finish});
        } else {
            this._shown = false;
            finish();
        }
        this._veils = [];
        // The glass stays under a pane while it fades
        this._module.surfacesChanged();
    }

    // --- keys and the pointer -------------------------------------------------------------------------------------

    _keyPress(event) {
        const key = event.get_key_symbol();
        const state = event.get_state();
        const action = global.display.get_keybinding_action(event.get_key_code(), state);
        const ctrl = state & Clutter.ModifierType.CONTROL_MASK;
        const backward = [Meta.KeyBindingAction.SWITCH_WINDOWS_BACKWARD,
            Meta.KeyBindingAction.SWITCH_APPLICATIONS_BACKWARD].includes(action);
        const forward = [Meta.KeyBindingAction.SWITCH_WINDOWS,
            Meta.KeyBindingAction.SWITCH_APPLICATIONS].includes(action);
        if (forward || backward || key === Clutter.KEY_Tab || key === Clutter.KEY_ISO_Left_Tab) {
            const back = backward || key === Clutter.KEY_ISO_Left_Tab || (state & Clutter.ModifierType.SHIFT_MASK);
            this._select(this._selected + (back ? -1 : 1));
            this._showNow();
            return Clutter.EVENT_STOP;
        }
        switch (key) {
        case Clutter.KEY_Escape:
            this.close();
            return Clutter.EVENT_STOP;
        case Clutter.KEY_Return:
        case Clutter.KEY_KP_Enter:
        case Clutter.KEY_ISO_Enter:
            if (this._shownTiles.length)
                this._finish();
            return Clutter.EVENT_STOP;
        case Clutter.KEY_Left:
        case Clutter.KEY_Right: {
            const rtl = Clutter.get_default_text_direction() === Clutter.TextDirection.RTL;
            this._select(this._selected + ((key === Clutter.KEY_Right) !== rtl ? 1 : -1));
            this._showNow();
            return Clutter.EVENT_STOP;
        }
        case Clutter.KEY_Up:
        case Clutter.KEY_Down: {
            const next = this._selected + (key === Clutter.KEY_Down ? this._columns : -this._columns);
            if (next >= 0 && next < this._shownTiles.length)
                this._select(next);
            this._showNow();
            return Clutter.EVENT_STOP;
        }
        case Clutter.KEY_BackSpace:
            if (this._query) {
                this._query = ctrl ? '' : [...this._query].slice(0, -1).join('');
                this._filter();
            }
            return Clutter.EVENT_STOP;
        case Clutter.KEY_space:
            if (!this._query) {
                if (this._shownTiles.length)
                    this._finish();
                return Clutter.EVENT_STOP;
            }
        }
        // Anything printable is part of the query; a key of a shortcut with Ctrl is not
        const unicode = Clutter.keysym_to_unicode(key);
        if (!ctrl && unicode >= 32 && unicode !== 127)
            this._add(String.fromCodePoint(unicode));
        return Clutter.EVENT_STOP;
    }

    _keyRelease() {
        if (this._typed || !this._mask)
            return Clutter.EVENT_STOP;
        const [, , mods] = global.get_pointer();
        if (!(mods & this._mask))
            this._finish();
        return Clutter.EVENT_STOP;
    }

    _buttonPress(event) {
        const [x, y] = event.get_coords();
        const e = this._pane?.get_transformed_extents();
        const inside = e && x >= e.origin.x && x < e.origin.x + e.size.width &&
            y >= e.origin.y && y < e.origin.y + e.size.height;
        if (!inside)
            this.close();
        return Clutter.EVENT_PROPAGATE;
    }
}
