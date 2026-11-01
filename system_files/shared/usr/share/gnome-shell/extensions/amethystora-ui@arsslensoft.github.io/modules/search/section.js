// How the search draws the results of Amethystora's own providers (sums, settings, windows, the manual, the clipboard,
// the agent), in the search box and in the overview alike: a section with its title over its rows, each row a picture,
// a title, a short note after it and, at its end, a hint of what Enter does or of its Ctrl key. Every text a row shows
// is plain text, whoever wrote it (a window's title, what was copied): never read as markup.
//
// A section is what GNOME's search view calls a provider's display: providers.js gives it to the provider before the
// provider is added, so that GNOME draws no grid of its own for it.

import Atk from 'gi://Atk';
import Clutter from 'gi://Clutter';
import GLib from 'gi://GLib';
import GObject from 'gi://GObject';
import Meta from 'gi://Meta';
import Pango from 'gi://Pango';
import St from 'gi://St';

import * as Search from 'resource:///org/gnome/shell/ui/search.js';
import {ensureActorVisibleInScrollView} from 'resource:///org/gnome/shell/misc/animationUtils.js';
import {gettext as _} from 'resource:///org/gnome/shell/extensions/extension.js';

const PICTURE = 28;
const SYMBOL = 16;
const PIN = 12;

// A label that never reads markup and ends with an ellipsis when there is no room
function plainLabel(styleClass, text = '') {
    const label = new St.Label({style_class: styleClass, text, y_align: Clutter.ActorAlign.CENTER});
    label.clutter_text.use_markup = false;
    label.clutter_text.ellipsize = Pango.EllipsizeMode.END;
    return label;
}

// Text that has to read left to right in every language (a number, an id, a command)
function leftToRight(label) {
    label.text_direction = Clutter.TextDirection.LTR;
    label.clutter_text.text_direction = Clutter.TextDirection.LTR;
    return label;
}

// The picture at a row's start: what the provider made (an app's icon, a thumbnail), else a rounded box with a
// symbolic icon or two letters in it
function picture(meta) {
    const made = meta.createIcon?.(PICTURE);
    if (made)
        return made;
    const box = new St.Bin({
        style_class: meta.pictureClass ? `amethystora-search-pic ${meta.pictureClass}` : 'amethystora-search-pic',
        y_align: Clutter.ActorAlign.CENTER,
    });
    if (meta.letters) {
        box.child = leftToRight(new St.Label({
            style_class: 'amethystora-search-pic-letters', text: meta.letters, x_align: Clutter.ActorAlign.CENTER,
            y_align: Clutter.ActorAlign.CENTER,
        }));
    } else if (meta.gicon) {
        box.child = new St.Icon({
            gicon: meta.gicon, icon_size: meta.giconSize ?? SYMBOL, style_class: 'amethystora-search-pic-icon',
        });
    } else if (meta.symbol) {
        box.child = new St.Icon({icon_name: meta.symbol, icon_size: SYMBOL, style_class: 'amethystora-search-pic-icon'});
    }
    return box;
}

// One result: GNOME's own result button, so that Enter, a click, the copy of clipboardText and the overview's
// closing are GNOME's; with our look and our hint at its end
export const Row = GObject.registerClass(
class AmethystoraSearchRow extends Search.SearchResult {
    _init(provider, meta, view, section) {
        super._init(provider, meta, view);
        this._section = section;
        this._place = 0;
        this.x_expand = true;
        const answer = meta.answer === true;
        this.style_class = answer ? 'amethystora-search-answer' : 'amethystora-search-row';

        const content = new St.BoxLayout({
            style_class: answer ? 'amethystora-search-answer-content' : 'amethystora-search-row-content',
            x_expand: true,
        });
        this.set_child(content);

        if (answer) {
            this._title = leftToRight(plainLabel('amethystora-search-answer-value', meta.name));
            this._title.y_align = Clutter.ActorAlign.END;
            content.add_child(this._title);
            // What the answer is about, always shown ("in Tokyo, the same day"); the hint after it
            if (meta.description) {
                const note = plainLabel('amethystora-search-answer-hint amethystora-search-answer-note', meta.description);
                note.y_align = Clutter.ActorAlign.END;
                content.add_child(note);
            }
            this._hint = plainLabel('amethystora-search-answer-hint');
            this._hint.y_align = Clutter.ActorAlign.END;
            content.add_child(this._hint);
            this.accessible_name = meta.description
                ? `${meta.expression} = ${meta.name}, ${meta.description}` : `${meta.expression} = ${meta.name}`;
        } else {
            content.add_child(picture(meta));
            this._title = plainLabel('amethystora-search-row-title', meta.name);
            content.add_child(this._title);
            if (meta.pinned) {
                content.add_child(new St.Icon({
                    icon_name: 'view-pin-symbolic', icon_size: PIN, style_class: 'amethystora-search-row-pin',
                    y_align: Clutter.ActorAlign.CENTER,
                }));
            }
            if (meta.description) {
                const note = plainLabel('amethystora-search-row-meta', meta.description);
                if (meta.descriptionLtr)
                    leftToRight(note);
                content.add_child(note);
                this.get_accessible().add_relationship(Atk.RelationType.DESCRIBED_BY, note.get_accessible());
            }
            content.add_child(new St.Widget({x_expand: true}));
            this._hint = plainLabel('amethystora-search-row-hint');
            content.add_child(this._hint);
        }
        this.label_actor = this._title;

        // The hint says what Enter does while the row is the one Enter opens, else its Ctrl key; nothing is asked of a
        // row on its way out (losing the focus as it goes)
        this.connect('destroy', () => (this._gone = true));
        this.connect('notify::pseudo-class', () => this.syncHint());
        this.connect('key-focus-in', () => {
            this.syncHint();
            if (!this._gone)
                section._focused(this);
        });
        this.connect('key-focus-out', () => this.syncHint());
        this.syncHint();
    }

    // Its place among every result shown, from 1; 0 when it has no Ctrl key (in the overview, or after the ninth)
    set place(value) {
        if (this._place === value || this._gone)
            return;
        this._place = value;
        this.syncHint();
    }

    get selected() {
        return this.has_style_pseudo_class('selected') || this.has_key_focus();
    }

    syncHint() {
        if (this._gone)
            return;
        const text = this.selected ? this.metaInfo.hint ?? '' : this._place > 0 ? _('Ctrl %d').format(this._place) : '';
        if (this._hint.text !== text)
            this._hint.text = text;
        this._hint.visible = text !== '';
    }

    get hintText() {
        return this._hint.visible ? this._hint.text : '';
    }
});

// One provider's results: its title (and a tag after it), its rows, and a short line under them
export const Section = GObject.registerClass({
    Properties: {
        'focus-child': GObject.ParamSpec.object('focus-child', null, null, GObject.ParamFlags.READABLE,
            Clutter.Actor.$gtype),
    },
}, class AmethystoraSearchSection extends St.BoxLayout {
    _init(provider, view, {title = '', tag = '', note = '', max = 5, locale = undefined} = {}) {
        super._init({
            style_class: 'search-section amethystora-search-section',
            orientation: Clutter.Orientation.VERTICAL,
            x_expand: true,
            visible: false,
        });
        this.provider = provider;
        this._view = view;
        this._max = max;
        this._serial = 0;
        this._focusChild = null;
        this._rows = [];
        this._leaving = [];
        this._leaveLater = 0;
        this.connect('destroy', () => {
            this._gone = true;
            if (this._leaveLater)
                global.compositor.get_laters().remove(this._leaveLater);
            this._leaveLater = 0;
            this._destroyLeaving();
        });

        if (title) {
            const header = new St.BoxLayout({style_class: 'amethystora-search-section-header', x_expand: true});
            // Short words: never cut (St adds the letter spacing after Pango has measured them)
            const words = (styleClass, text) => {
                const label = plainLabel(styleClass, text.toLocaleUpperCase(locale));
                label.clutter_text.ellipsize = Pango.EllipsizeMode.NONE;
                return label;
            };
            header.add_child(words('amethystora-search-section-title', title));
            if (tag)
                header.add_child(words('amethystora-search-section-tag', tag));
            this.add_child(header);
        }
        // In the overview the rows sit on GNOME's own section card; the search box takes the card away
        this._list = new St.BoxLayout({
            style_class: 'search-section-content amethystora-search-rows',
            orientation: Clutter.Orientation.VERTICAL,
            x_expand: true,
        });
        this.add_child(this._list);
        if (note)
            this.add_child(plainLabel('amethystora-search-section-note', note));
    }

    get focusChild() {
        return this._focusChild;
    }

    get rows() {
        return this._rows;
    }

    _focused(row) {
        if (this._focusChild !== row) {
            this._focusChild = row;
            this.notify('focus-child');
        }
        // GNOME scrolls to the focused result of the displays it makes; this one is ours
        let scrollView = this.get_parent();
        while (scrollView && !(scrollView instanceof St.ScrollView))
            scrollView = scrollView.get_parent();
        if (scrollView) {
            try {
                ensureActorVisibleInScrollView(scrollView, row);
            } catch {}
        }
    }

    async updateSearch(ids, terms) {
        const serial = ++this._serial;
        const shown = this.provider.filterResults(ids, this._max);
        if (shown.length === 0) {
            this._retireRows();
            this.hide();
            return;
        }
        let metas;
        try {
            metas = await this.provider.getResultMetas(shown, null);
        } catch (e) {
            if (serial === this._serial && !this._gone)
                this.clear();
            console.warn(`Amethystora UI: the ${this.provider.id} search results: ${e.message}`);
            return;
        }
        // A newer search began meanwhile, or the section went (its rows would queue a later after its own was removed)
        if (serial !== this._serial || this._gone)
            return;
        this._terms = terms;
        this._retireRows();
        for (const meta of metas) {
            if (!meta?.id || !meta.name)
                continue;
            const row = new Row(this.provider, meta, this._view, this);
            this._rows.push(row);
            this._list.add_child(row);
        }
        this.visible = this._rows.length > 0;
    }

    // The rows of the last search leave the list at once, but are destroyed only before the next frame: GNOME's view
    // still holds the one Enter opened until it picks the new one
    _retireRows() {
        this._focusChild = null;
        for (const row of this._rows) {
            this._list.remove_child(row);
            this._leaving.push(row);
        }
        this._rows = [];
        if (this._leaving.length && !this._leaveLater) {
            this._leaveLater = global.compositor.get_laters().add(Meta.LaterType.BEFORE_REDRAW, () => {
                this._leaveLater = 0;
                this._destroyLeaving();
                return GLib.SOURCE_REMOVE;
            });
        }
    }

    _destroyLeaving() {
        for (const row of this._leaving)
            row.destroy();
        this._leaving = [];
    }

    _clearRows() {
        this._focusChild = null;
        for (const row of this._rows)
            row.destroy();
        this._rows = [];
        this._destroyLeaving();
    }

    clear() {
        this._serial++;
        this._clearRows();
        this.hide();
    }

    getFirstResult() {
        return this._rows[0] ?? null;
    }
});
