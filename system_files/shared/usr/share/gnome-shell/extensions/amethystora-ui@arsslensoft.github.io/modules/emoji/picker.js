// The emoji and symbols picker's card: a search field, the tabs, a grid of six columns that scrolls, and a line saying
// which keys do what. It is built when the picker opens and destroyed when it closes. Picking (a click, Enter) hands
// the character to the module, which closes the card and puts it where the person was typing. A character that comes
// in skin tones is drawn in the chosen one; a right click, a long press or the Menu key on it opens a strip of its six.

import Clutter from 'gi://Clutter';
import GLib from 'gi://GLib';
import GObject from 'gi://GObject';
import Meta from 'gi://Meta';
import St from 'gi://St';

import {ensureActorVisibleInScrollView} from 'resource:///org/gnome/shell/misc/animationUtils.js';
import {gettext as _} from 'resource:///org/gnome/shell/extensions/extension.js';

import {TABS, TONES} from './data.js';
import {WrapLayout} from './wrap.js';

const COLUMNS = 6;
// Cells made in each frame, so that the first rows are there in the first frame of a large tab
const CHUNK = 60;
const LONG_PRESS = 500;
const GAP = 4;

const Cell = GObject.registerClass(
class AmethystoraEmojiCell extends St.Button {
    _init(character, name) {
        super._init({
            style_class: 'amethystora-emoji-cell',
            label: character,
            can_focus: true,
            reactive: true,
            track_hover: true,
            x_expand: true,
            accessible_name: name || character,
        });
        this.character = character;
        this.get_child()?.clutter_text?.set_use_markup(false);
    }
});

export class Picker {
    // owner: the module; {data, tab, tone, recent} what to show
    constructor(owner, {tab, recent, tone}) {
        this._owner = owner;
        this._data = null;
        this._recent = recent;
        this._tone = tone;
        this._tab = tab;
        this._cells = [];
        this._queue = [];
        this._chunkLater = 0;
        const rtl = Clutter.get_default_text_direction() === Clutter.TextDirection.RTL;
        this._rtl = rtl;
        // Spacing set from code is in the screen's pixels
        const gap = GAP * St.ThemeContext.get_for_stage(global.stage).scale_factor;

        this.actor = new St.BoxLayout({
            style_class: 'amethystora-emoji-picker',
            orientation: Clutter.Orientation.VERTICAL,
            reactive: true,
        });

        this._entry = new St.Entry({
            style_class: 'amethystora-emoji-search',
            hint_text: _('Search: arrow, euro, smile…'),
            can_focus: true,
            x_expand: true,
        });
        this._entry.set_primary_icon(new St.Icon({
            style_class: 'amethystora-emoji-search-icon', icon_name: 'edit-find-symbolic',
        }));
        this._entry.clutter_text.connectObject('text-changed', () => this._filter(), this);
        this.actor.add_child(this._entry);

        this._tabsBox = new St.Widget({
            style_class: 'amethystora-emoji-tabs', layout_manager: new WrapLayout(gap), x_expand: true,
        });
        this.actor.add_child(this._tabsBox);
        this._tabButtons = new Map();

        const grid = new Clutter.GridLayout({column_spacing: gap, row_spacing: gap, column_homogeneous: true});
        this._grid = new St.Viewport({style_class: 'amethystora-emoji-grid-content', layout_manager: grid, x_expand: true});
        this._gridLayout = grid;
        this._scroll = new St.ScrollView({
            style_class: 'amethystora-emoji-grid',
            hscrollbar_policy: St.PolicyType.NEVER,
            vscrollbar_policy: St.PolicyType.AUTOMATIC,
            overlay_scrollbars: true,
            x_expand: true,
            child: this._grid,
        });
        this.actor.add_child(this._scroll);

        const footer = new St.BoxLayout({style_class: 'amethystora-emoji-footer'});
        const chip = text => new St.Label({style_class: 'amethystora-emoji-key', text, y_align: Clutter.ActorAlign.CENTER});
        const words = text => new St.Label({text, y_align: Clutter.ActorAlign.CENTER});
        footer.add_child(chip(_('Enter')));
        footer.add_child(words(_('pastes')));
        footer.add_child(words('·'));
        footer.add_child(chip(_('Esc')));
        footer.add_child(words(_('closes')));
        this.actor.add_child(footer);

        // Keys before the field or a cell see them
        this._keys = new Clutter.KeyController();
        this._keys.connectObject('key-press', controller => {
            const [, keyval, , unicode] = controller.get_key();
            const [, state] = controller.get_state();
            return this._onKey(keyval, state, unicode);
        }, this);
        this.actor.add_action_full('amethystora-emoji-keys', Clutter.EventPhase.CAPTURE, this._keys);
        this._makeTabs();
    }

    get entry() {
        return this._entry;
    }

    get tab() {
        return this._query ? '' : this._tab;
    }

    get query() {
        return this._query ?? '';
    }

    get shown() {
        return this._characters ?? [];
    }

    get strip() {
        return this._strip ?? null;
    }

    contains(actor) {
        return this.actor.contains(actor) || (this._strip?.contains(actor) ?? false);
    }

    _tabTitle(tab) {
        return {
            recent: _('Recent'), smileys: _('Smileys'), arrows: _('Arrows'), maths: _('Maths'), money: _('Money'),
            punctuation: _('Punctuation'), people: _('People'), nature: _('Nature'), food: _('Food'),
            places: _('Places'), activities: _('Activities'), objects: _('Objects'), symbols: _('Symbols'),
            flags: _('Flags'),
        }[tab];
    }

    _makeTabs() {
        this._tabsBox.destroy_all_children();
        this._tabButtons.clear();
        for (const tab of TABS) {
            if (tab === 'recent' && this._recent.length === 0)
                continue;
            const button = new St.Button({
                style_class: 'amethystora-emoji-tab', label: this._tabTitle(tab), can_focus: true,
            });
            button.connectObject('clicked', () => this.showTab(tab), this);
            this._tabButtons.set(tab, button);
            this._tabsBox.add_child(button);
        }
        this._syncTabs();
    }

    _syncTabs() {
        for (const [tab, button] of this._tabButtons) {
            if (tab === this._tab && !this._query)
                button.add_style_pseudo_class('checked');
            else
                button.remove_style_pseudo_class('checked');
        }
    }

    // The data is in: the grid fills
    setData(data) {
        this._data = data;
        this._filter();
    }

    setTone(tone) {
        this._tone = tone;
        this._fill(this._characters ?? []);
    }

    showTab(tab) {
        if (!this._tabButtons.has(tab))
            return;
        this._tab = tab;
        if (this._entry.get_text())
            this._entry.set_text('');
        else
            this._filter();
    }

    _filter() {
        const text = this._entry.get_text().trim();
        this._query = text;
        this._syncTabs();
        if (!this._data) {
            this._fill([]);
            return;
        }
        let characters;
        if (text)
            characters = this._data.search(text);
        else if (this._tab === 'recent')
            characters = this._recent;
        else
            characters = this._data.tabs.get(this._tab) ?? [];
        this._fill(characters);
    }

    // The grid shows these characters, made a chunk a frame
    _fill(characters) {
        this._closeStrip();
        this._characters = characters;
        this._stopChunks();
        this._grid.destroy_all_children();
        this._cells = [];
        this._queue = characters.map((character, index) => [character, index]);
        this._scroll.vadjustment.value = 0;
        this._chunk();
    }

    _chunk() {
        this._chunkLater = 0;
        const now = this._queue.splice(0, CHUNK);
        for (const [character, index] of now) {
            // Recent picks are kept as picked; the rest in the chosen tone
            const shown = this._query || this._tab !== 'recent' ? this._data.toned(character, this._tone) : character;
            const cell = new Cell(shown, this._data.nameOf(character));
            cell.base = character;
            cell.connectObject('clicked', () => this._owner.pick(cell.character), this);
            if (this._data.tones.has(character)) {
                const right = new Clutter.ClickGesture({required_button: Clutter.BUTTON_SECONDARY, recognize_on_press: true});
                right.connect('recognize', () => this._openStrip(cell));
                cell.add_action(right);
                const hold = new Clutter.LongPressGesture({long_press_duration_ms: LONG_PRESS});
                hold.connect('recognize', () => this._openStrip(cell));
                cell.add_action(hold);
            }
            this._gridLayout.attach(cell, index % COLUMNS, Math.floor(index / COLUMNS), 1, 1);
            this._cells.push(cell);
        }
        if (this._queue.length) {
            this._chunkLater = global.compositor.get_laters().add(Meta.LaterType.BEFORE_REDRAW, () => {
                this._chunk();
                return GLib.SOURCE_REMOVE;
            });
        }
    }

    _stopChunks() {
        if (this._chunkLater)
            global.compositor.get_laters().remove(this._chunkLater);
        this._chunkLater = 0;
        this._queue = [];
    }

    // ---- Keys ----

    _focusIndex() {
        const focus = global.stage.get_key_focus();
        return this._cells.findIndex(cell => cell === focus);
    }

    _focusCell(index) {
        const cell = this._cells[index];
        if (!cell)
            return false;
        cell.grab_key_focus();
        ensureActorVisibleInScrollView(this._scroll, cell);
        return true;
    }

    _focusGroup(step) {
        const focus = global.stage.get_key_focus();
        const groups = [this._entry.clutter_text, this._tabButtons.get(this._tab) ?? [...this._tabButtons.values()][0],
            this._cells[Math.max(0, this._focusIndex())] ?? this._cells[0]].filter(Boolean);
        let at = groups.findIndex(group => group === focus || (group === this._entry.clutter_text && this._entry.contains(focus)));
        if (at < 0)
            at = this._tabsBox.contains(focus) ? 1 : 2;
        const next = groups[(at + step + groups.length) % groups.length];
        if (next === this._entry.clutter_text)
            this._entry.grab_key_focus();
        else
            next?.grab_key_focus();
    }

    _onKey(keyval, state, unicode) {
        const control = (state & Clutter.ModifierType.CONTROL_MASK) !== 0;
        const focus = global.stage.get_key_focus();
        const inField = this._entry.contains(focus);
        const index = this._focusIndex();

        switch (keyval) {
        case Clutter.KEY_Escape:
            this._owner.close();
            return Clutter.EVENT_STOP;
        case Clutter.KEY_Page_Down:
        case Clutter.KEY_Page_Up: {
            if (!control)
                return Clutter.EVENT_PROPAGATE;
            const tabs = [...this._tabButtons.keys()];
            const at = Math.max(0, tabs.indexOf(this._tab));
            this.showTab(tabs[(at + (keyval === Clutter.KEY_Page_Down ? 1 : -1) + tabs.length) % tabs.length]);
            return Clutter.EVENT_STOP;
        }
        case Clutter.KEY_Tab:
        case Clutter.KEY_ISO_Left_Tab:
            this._focusGroup(keyval === Clutter.KEY_ISO_Left_Tab ? -1 : 1);
            return Clutter.EVENT_STOP;
        case Clutter.KEY_Return:
        case Clutter.KEY_KP_Enter:
        case Clutter.KEY_ISO_Enter:
            if (index >= 0)
                this._owner.pick(this._cells[index].character);
            else if (inField && this._cells[0])
                this._owner.pick(this._cells[0].character);
            else
                return Clutter.EVENT_PROPAGATE;
            return Clutter.EVENT_STOP;
        case Clutter.KEY_Menu:
            if (index >= 0 && this._data?.tones.has(this._cells[index].base)) {
                this._openStrip(this._cells[index]);
                return Clutter.EVENT_STOP;
            }
            return Clutter.EVENT_PROPAGATE;
        case Clutter.KEY_Down:
            if (inField || this._tabsBox.contains(focus))
                return this._focusCell(0) ? Clutter.EVENT_STOP : Clutter.EVENT_PROPAGATE;
            if (index >= 0)
                this._focusCell(Math.min(index + COLUMNS, this._cells.length - 1));
            return Clutter.EVENT_STOP;
        case Clutter.KEY_Up:
            if (index >= 0) {
                if (index < COLUMNS)
                    this._entry.grab_key_focus();
                else
                    this._focusCell(index - COLUMNS);
                return Clutter.EVENT_STOP;
            }
            return Clutter.EVENT_PROPAGATE;
        case Clutter.KEY_Left:
        case Clutter.KEY_Right: {
            const forward = (keyval === Clutter.KEY_Right) !== this._rtl;
            if (index >= 0) {
                this._focusCell(Math.max(0, Math.min(this._cells.length - 1, index + (forward ? 1 : -1))));
                return Clutter.EVENT_STOP;
            }
            if (this._tabsBox.contains(focus)) {
                const buttons = [...this._tabButtons.values()];
                const at = buttons.indexOf(focus);
                buttons[Math.max(0, Math.min(buttons.length - 1, at + (forward ? 1 : -1)))]?.grab_key_focus();
                return Clutter.EVENT_STOP;
            }
            return Clutter.EVENT_PROPAGATE;
        }
        }

        // A letter typed with the keyboard on a cell or a tab goes into the field
        if (!inField && unicode && !control && !(state & Clutter.ModifierType.MOD1_MASK) && unicode.trim()) {
            this._entry.grab_key_focus();
            this._entry.set_text(`${this._entry.get_text()}${unicode}`);
            this._entry.clutter_text.set_cursor_position(-1);
            return Clutter.EVENT_STOP;
        }
        return Clutter.EVENT_PROPAGATE;
    }

    // ---- The strip of skin tones ----

    _openStrip(cell) {
        this._closeStrip();
        const base = cell.base;
        if (!this._data?.tones.has(base))
            return;
        this._strip = new St.BoxLayout({style_class: 'amethystora-emoji-tones', reactive: true});
        TONES.forEach(tone => {
            const variant = this._data.toned(base, tone);
            const button = new Cell(variant, this._data.nameOf(base));
            button.x_expand = false;
            button.width = cell.width;
            button.add_style_class_name('amethystora-emoji-tone');
            button.connectObject('clicked', () => this._owner.pick(variant, tone), this);
            this._strip.add_child(button);
        });
        this._stripFor = cell;
        // The strip is not inside the card: it hears its own keys
        const keys = new Clutter.KeyController();
        keys.connect('key-press', controller => this._onStripKey(controller.get_key()[1]));
        this._strip.add_action_full('amethystora-emoji-tone-keys', Clutter.EventPhase.CAPTURE, keys);
        this._owner.showStrip(this._strip, cell);
        this._strip.get_child_at_index(Math.max(0, TONES.indexOf(this._tone)))?.grab_key_focus();
    }

    _onStripKey(keyval) {
        const buttons = this._strip.get_children();
        const at = buttons.indexOf(global.stage.get_key_focus());
        switch (keyval) {
        case Clutter.KEY_Escape: {
            const cell = this._stripFor;
            this._closeStrip();
            cell?.grab_key_focus();
            return Clutter.EVENT_STOP;
        }
        case Clutter.KEY_Left:
        case Clutter.KEY_Right: {
            const forward = (keyval === Clutter.KEY_Right) !== this._rtl;
            buttons[Math.max(0, Math.min(buttons.length - 1, at + (forward ? 1 : -1)))]?.grab_key_focus();
            return Clutter.EVENT_STOP;
        }
        case Clutter.KEY_Return:
        case Clutter.KEY_KP_Enter:
            if (at >= 0)
                buttons[at].emit('clicked', 1);
            return Clutter.EVENT_STOP;
        }
        return Clutter.EVENT_PROPAGATE;
    }

    _closeStrip() {
        if (!this._strip)
            return;
        const strip = this._strip;
        this._strip = null;
        this._stripFor = null;
        this._owner.hideStrip(strip);
        strip.destroy();
    }

    destroy() {
        this._stopChunks();
        this._closeStrip();
        this._keys?.disconnectObject(this);
        this._entry?.clutter_text.disconnectObject(this);
        this.actor.destroy();
        this._cells = [];
        this._data = null;
    }
}
