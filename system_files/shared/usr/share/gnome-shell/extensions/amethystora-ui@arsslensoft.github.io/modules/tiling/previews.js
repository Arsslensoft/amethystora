// The workspaces as cards while Super is held: one above the other in their order for Super+Page Up and Page
// Down, or in a pile, the most recent in front, for going back through the recent ones (Super+Ctrl+grave). Each
// card is the workspace as it is, its wallpaper (GNOME's own, from its shared cache) and its windows. Letting Super
// go zooms the chosen card to the whole screen, and that workspace is the one shown.

import Clutter from 'gi://Clutter';
import St from 'gi://St';

import * as Background from 'resource:///org/gnome/shell/ui/background.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';

import {config} from './config.js';
import {backgroundActor, colourOf, nameOf} from './backgrounds.js';

// The frame round each card is one of this many colours of the theme, one per workspace in turn (shell-tiling.css)
const TONES = 14;
// The cards in order: their size, and the space between two, as shares of the screen
const ROW_SCALE = 0.825;
const ROW_SPACE = 0.04;
// The pile: the chosen card's size and top, and where the ones behind it and the ones passed show
const PILE_SCALE = 0.9;
const PILE_TOP = 0.10;
const PILE_BEHIND = [0.035, 0.01];
const PILE_PASSED = [0.95, 1.10];
const PILE_SHRINK = 0.99;
// The frame's width, as the stylesheet has it
const FRAME = 8;

// How many sets of cards are up: the top bar is shown meanwhile, and its icons take no clicks
let open = 0;

export function cardsShown() {
    return open > 0;
}

export class WorkspacePreview {
    // kind: 'row' or 'pile'; workspaces in the order the cards go; selected: the index of the chosen one;
    // carried: the windows being carried, which are on no card
    constructor(engine, monitor, kind, workspaces, selected, carried = []) {
        this._engine = engine;
        this._carried = new Set(carried);
        this._monitor = Main.layoutManager.monitors[monitor];
        this._monitorIndex = monitor;
        this._kind = kind;
        this.workspaces = workspaces;
        this.selected = selected;
        const m = this._monitor;
        this.actor = new St.Widget({
            style_class: 'amethystora-tiling-previews', reactive: false,
            x: m.x, y: m.y, width: m.width, height: m.height,
        });
        // Under the top bar, which stays in view meanwhile
        Main.layoutManager.uiGroup.insert_child_below(this.actor, Main.layoutManager.panelBox);
        this._cards = workspaces.map(workspace => this._card(workspace));
        engine.boxHidden++;
        open++;
        engine.tiling.topBar?.applyTopBar();
        this._place(false);
    }

    get workspace() {
        return this.workspaces[this.selected];
    }

    _card(workspace) {
        const m = this._monitor;
        // The frame is drawn round the workspace, which keeps the size of the screen inside it
        const content = new St.Widget({clip_to_allocation: true, width: m.width, height: m.height});
        const card = new St.Bin({style_class: 'amethystora-tiling-card', child: content});
        card.add_style_class_name(`amethystora-tiling-card-${workspace.index() % TONES}`);
        card._background = new Background.BackgroundManager({
            container: content, monitorIndex: this._monitorIndex,
            controlPosition: false,
        });
        // The workspace's own picture over GNOME's, and its own colour round it, when the settings give it them
        const own = backgroundActor(workspace);
        if (own) {
            own.set_size(m.width, m.height);
            content.add_child(own);
        }
        const colour = colourOf(workspace);
        if (colour)
            card.set_style(`border-color: ${colour};`);
        // The floating windows are on every workspace, and shown on the one they are seen on. With a workspace on
        // each screen, a workspace's windows are on the screen its strip is on, which may be another than this one.
        const active = global.workspace_manager.get_active_workspace();
        const home = this._engine.screens.separate ? this._engine.strip(workspace, this._monitorIndex).monitor
            : this._monitorIndex;
        const origin = Main.layoutManager.monitors[home] ?? m;
        const actors = global.get_window_actors().filter(actor => {
            const window = actor.meta_window;
            if (this._carried.has(window) || (window.is_on_all_workspaces() && workspace !== active))
                return false;
            const monitor = window.is_on_all_workspaces() ? this._monitorIndex : home;
            return window.located_on_workspace(workspace) && !window.minimized &&
                window.get_monitor() === monitor && !window.is_override_redirect();
        });
        for (const actor of actors) {
            const window = actor.meta_window;
            const info = this._engine.info(window);
            if (info?.hidden)
                continue;
            const frame = window.get_frame_rect();
            const buffer = window.get_buffer_rect();
            const at = info?.strip && info.vis ? info.vis : frame;
            const from = window.is_on_all_workspaces() ? m : origin;
            const clone = new Clutter.Clone({source: actor});
            clone.set_position(at.x + buffer.x - frame.x - from.x, at.y + buffer.y - frame.y - from.y);
            content.add_child(clone);
        }
        // With the workspace's name shown in the top bar, its card has it too, over its windows
        if (config.indicator) {
            content.add_child(new St.Label({
                style_class: 'amethystora-tiling-card-name', text: nameOf(workspace), x: 24, y: 24,
            }));
        }
        this.actor.add_child(card);
        return card;
    }

    // Each card where it belongs for the chosen one
    _place(animate = true) {
        const m = this._monitor;
        const n = this._cards.length;
        const s = this.selected;
        this._cards.forEach((card, i) => {
            let scale, top, visible = true;
            if (this._kind === 'row') {
                scale = ROW_SCALE;
                const height = m.height * scale;
                const space = m.height * ROW_SPACE;
                // The first card at the top, the last at the bottom, any other in the middle
                let share = 0.5;
                if (s === 0)
                    share = 0;
                else if (s === n - 1)
                    share = 1;
                const chosenTop = space + (m.height - height - 2 * space) * share;
                top = chosenTop + (i - s) * (height + space);
            } else {
                const d = i - s;
                scale = PILE_SCALE * PILE_SHRINK ** i;
                if (d === 0)
                    top = m.height * PILE_TOP;
                else if (d > 0)
                    [top, visible] = d <= PILE_BEHIND.length ? [m.height * PILE_BEHIND[d - 1], true] : [0, false];
                else
                    top = m.height * PILE_PASSED[Math.min(-d, PILE_PASSED.length) - 1];
            }
            const x = Math.round((m.width - (m.width + 2 * FRAME) * scale) / 2);
            const params = {x, y: Math.round(top), scale_x: scale, scale_y: scale, opacity: visible ? 255 : 0};
            if (animate)
                card.ease({...params, duration: config.animation, mode: Clutter.AnimationMode.EASE_OUT_QUAD});
            else
                card.set(params);
        });
        // The ones further back in the pile behind the ones in front
        if (this._kind === 'pile') {
            for (let i = this._cards.length - 1; i >= 0; i--)
                this.actor.set_child_above_sibling(this._cards[i], null);
        }
    }

    // One card on: in the row it stops at either end, in the pile it goes round
    step(direction) {
        const n = this._cards.length;
        const next = this._kind === 'pile' ? (this.selected + direction + n) % n : this.selected + direction;
        if (next < 0 || next >= n || next === this.selected)
            return false;
        this.selected = next;
        this._place();
        return true;
    }

    // The chosen card grows to the whole screen, then its workspace is shown and the cards go
    finish(done) {
        const card = this._cards[this.selected];
        this.actor.set_child_above_sibling(card, null);
        this._cards.forEach((other, i) => {
            if (other !== card && this._kind === 'pile' && i > this.selected)
                other.ease({y: this._monitor.height * PILE_PASSED.at(-1), duration: config.animation});
        });
        card.ease({
            x: -FRAME, y: -FRAME, scale_x: 1, scale_y: 1, duration: config.animation, mode: Clutter.AnimationMode.EASE_OUT_QUAD,
            onStopped: () => {
                // The cards go whatever `done` does: the tiling may have stopped during the zoom (the screen locked)
                try {
                    done(this.workspace);
                } finally {
                    this.destroy();
                }
            },
        });
    }

    destroy() {
        if (!this.actor)
            return;
        for (const card of this._cards)
            card._background?.destroy();
        this.actor.destroy();
        this.actor = null;
        this._engine.boxHidden--;
        open--;
        this._engine.tiling?.topBar?.applyTopBar();
        for (const strip of this._engine.strips?.() ?? [])
            this._engine.updateBox(strip);
    }
}

// Shows a workspace on a screen without GNOME's own sliding animation: the preview has already shown the way there
export function activateQuietly(engine, workspace, monitor) {
    engine.screens.bring(workspace, monitor);
}
