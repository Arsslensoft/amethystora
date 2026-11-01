// The Focus tile in quick settings, as board 5 and the Focus board draw it: a block across both columns with Focus and
// what it is doing, the four chips (Off, Work, Games, Sleep), what the chosen profile does, and when it ends, with a
// way to Control, where each profile is edited. A chip picks a profile by hand; an end picked while Off applies to the
// next profile picked here. The block is filled when quick settings opens and while it is open; closed, it is left as
// it is.

import Atk from 'gi://Atk';
import Clutter from 'gi://Clutter';
import GLib from 'gi://GLib';
import Meta from 'gi://Meta';
import St from 'gi://St';

import {gettext as _} from 'resource:///org/gnome/shell/extensions/extension.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import * as Util from 'resource:///org/gnome/shell/misc/util.js';

import {CHOICES, profileName} from './profiles.js';

const CONTROL = 'amethystora-control';

function label(text, styleClass, props = {}) {
    const actor = new St.Label({text, style_class: styleClass, y_align: Clutter.ActorAlign.CENTER, ...props});
    actor.clutter_text.use_markup = false;
    return actor;
}

export class FocusTile {
    constructor(focus) {
        this._focus = focus;
        this._chips = new Map();
        this._ends = new Map();

        this.block = new St.BoxLayout({
            style_class: 'amethystora-quick-block amethystora-focus-tile',
            orientation: Clutter.Orientation.VERTICAL,
            x_expand: true,
        });
        this.block._amethystoraQuickId = 'focus';

        const header = new St.BoxLayout({style_class: 'amethystora-focus-header', x_expand: true});
        header.add_child(label(_('Focus'), 'amethystora-quick-block-title amethystora-focus-title', {x_expand: true}));
        this._state = label('', 'amethystora-focus-state');
        header.add_child(this._state);
        this.block.add_child(header);

        const chips = new St.BoxLayout({style_class: 'amethystora-focus-chips', x_expand: true});
        for (const id of CHOICES) {
            const chip = new St.Button({
                style_class: 'amethystora-focus-chip',
                label: profileName(id),
                can_focus: true,
                accessible_role: Atk.Role.TOGGLE_BUTTON,
                accessible_name: profileName(id),
            });
            chip.clutter_text?.set_use_markup?.(false);
            chip.connect('clicked', () => this._focus.pick(id));
            this._chips.set(id, chip);
            chips.add_child(chip);
        }
        this.block.add_child(chips);

        this._lines = new St.BoxLayout({style_class: 'amethystora-focus-lines', orientation: Clutter.Orientation.VERTICAL,
            x_expand: true});
        this.block.add_child(this._lines);

        const ends = new St.BoxLayout({style_class: 'amethystora-focus-ends', x_expand: true});
        const choices = new St.BoxLayout({style_class: 'amethystora-focus-end-choices', x_expand: true,
            y_align: Clutter.ActorAlign.CENTER});
        choices.add_child(label(_('Ends:'), 'amethystora-focus-ends-label'));
        const words = [['off', _('when you turn it off')], ['hour', _('1 hour')], ['tomorrow', _('tomorrow')]];
        words.forEach(([id, text], i) => {
            if (i > 0)
                choices.add_child(label('·', 'amethystora-focus-ends-dot'));
            const button = new St.Button({style_class: 'amethystora-focus-end', label: text, can_focus: true,
                accessible_name: _('Ends %s').format(text)});
            button.clutter_text?.set_use_markup?.(false);
            button.connect('clicked', () => this._focus.pickEnd(id));
            this._ends.set(id, button);
            choices.add_child(button);
        });
        ends.add_child(choices);
        const edit = new St.Button({style_class: 'amethystora-focus-edit', label: _('Edit in Control'), can_focus: true,
            y_align: Clutter.ActorAlign.CENTER});
        edit.connect('clicked', () => {
            Main.panel.statusArea.quickSettings?.menu.close();
            Util.spawn([CONTROL, 'desktop', 'focus']);
        });
        ends.add_child(edit);
        this.block.add_child(ends);

        // A wide font (the terminal style's) leaves no room for both on one line: Edit in Control goes under the choices,
        // rather than the choices being cut off
        const fit = () => {
            // Taking the block away notifies its width too: nothing is queued for actors on their way out
            if (this._destroyed)
                return;
            this._fitId ||= global.compositor.get_laters().add(Meta.LaterType.BEFORE_REDRAW, () => {
                this._fitId = 0;
                // A later queued before the block went may still be run: it touches nothing destroyed
                if (this._destroyed)
                    return GLib.SOURCE_REMOVE;
                const [, wanted] = choices.get_preferred_width(-1);
                const [, editWanted] = edit.get_preferred_width(-1);
                const below = ends.width > 0 && wanted + editWanted + 8 > ends.width;
                ends.orientation = below ? Clutter.Orientation.VERTICAL : Clutter.Orientation.HORIZONTAL;
                return GLib.SOURCE_REMOVE;
            });
        };
        // The menu may take the block away without asking the tile: what is queued goes with it
        ends.connect('destroy', () => {
            this._destroyed = true;
            if (this._fitId)
                global.compositor.get_laters().remove(this._fitId);
            this._fitId = 0;
        });
        ends.connect('notify::width', fit);
        ends.connect('style-changed', fit);
    }

    destroy() {
        this._destroyed = true;
        if (this._fitId)
            global.compositor.get_laters().remove(this._fitId);
        this._fitId = 0;
        this.block.destroy();
        this._chips.clear();
        this._ends.clear();
    }

    // As the module says now: the chosen chip, its lines, the state and the end
    refresh({id, state, lines, end}) {
        for (const [chipId, chip] of this._chips) {
            const on = chipId === id;
            if (on) {
                chip.add_style_pseudo_class('checked');
                chip.add_accessible_state(Atk.StateType.CHECKED);
            } else {
                chip.remove_style_pseudo_class('checked');
                chip.remove_accessible_state(Atk.StateType.CHECKED);
            }
        }
        this._state.text = state;
        if (lines.join('\n') !== this._shownLines) {
            this._shownLines = lines.join('\n');
            this._lines.destroy_all_children();
            for (const text of lines) {
                const row = new St.BoxLayout({style_class: 'amethystora-focus-line', x_expand: true});
                row.add_child(new St.Widget({style_class: 'amethystora-focus-dot', y_align: Clutter.ActorAlign.CENTER}));
                const words = label(text, 'amethystora-focus-line-text', {x_expand: true});
                words.clutter_text.line_wrap = true;
                row.add_child(words);
                this._lines.add_child(row);
            }
        }
        for (const [endId, button] of this._ends) {
            if (endId === end)
                button.add_style_pseudo_class('checked');
            else
                button.remove_style_pseudo_class('checked');
        }
    }

    // For the suites: what a person reads on the tile
    snapshot() {
        return {
            chosen: [...this._chips].find(([, chip]) => chip.has_style_pseudo_class('checked'))?.[0] ?? null,
            state: this._state.text,
            lines: this._lines.get_children().map(row => row.get_last_child().text),
            end: [...this._ends].find(([, b]) => b.has_style_pseudo_class('checked'))?.[0] ?? null,
            width: this.block.width,
            height: this.block.height,
        };
    }

    click(id) {
        this._chips.get(id)?.emit('clicked', 1);
    }

    clickEnd(id) {
        this._ends.get(id)?.emit('clicked', 1);
    }
}
