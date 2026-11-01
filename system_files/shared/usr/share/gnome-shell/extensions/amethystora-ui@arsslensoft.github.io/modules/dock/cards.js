// The glass cards the dock opens over an item (previews, a stack, a folder, a drop on the phone): where one goes, and
// how it comes and goes. A card stands `gap` px beyond the pill's far edge, centred on the item where it rests, and
// kept 8 px inside the screen's work area.

import * as Main from 'resource:///org/gnome/shell/ui/main.js';

import {EASE, easeTo, scaleFactor} from './util.js';

const MARGIN = 8;

// Puts card (already in the chrome, its size known) by the item whose resting centre along the row is `along`
export function placeCard(card, dash, along, gap) {
    const sf = scaleFactor();
    const pill = dash.pillRect();
    const area = Main.layoutManager.getWorkAreaForMonitor(dash.monitorIndex);
    const monitor = Main.layoutManager.monitors[dash.monitorIndex] ?? area;
    // A dock that reserves no strip lies inside the work area: the screen bounds it instead
    const bounds = dash.reserves ? area : monitor;
    const [, , width, height] = card.get_preferred_size();
    const margin = MARGIN * sf;
    let x, y;
    if (dash.edgeName === 'bottom') {
        x = Math.clamp(along - width / 2, bounds.x + margin, bounds.x + bounds.width - width - margin);
        y = pill.y - gap * sf - height;
    } else {
        y = Math.clamp(along - height / 2, bounds.y + margin, bounds.y + bounds.height - height - margin);
        x = dash.edgeName === 'left' ? pill.x + pill.width + gap * sf : pill.x - gap * sf - width;
    }
    card.set_position(Math.round(x), Math.round(y));
}

// The offset of a card that is `px` nearer the screen's edge than its place
export function towardsEdge(dash, px) {
    const sf = scaleFactor();
    switch (dash.edgeName) {
    case 'left': return {translation_x: -px * sf, translation_y: 0};
    case 'right': return {translation_x: px * sf, translation_y: 0};
    default: return {translation_x: 0, translation_y: px * sf};
    }
}

// The pivot on the side of the card that faces the dock
export function pivotTowardsDock(card, dash) {
    if (dash.edgeName === 'left')
        card.set_pivot_point(0, 0.5);
    else if (dash.edgeName === 'right')
        card.set_pivot_point(1, 0.5);
    else
        card.set_pivot_point(0.5, 1);
}

// Comes out of the dock: from `from` px nearer the edge, at `scale`, clear, to its place
export function popIn(card, dash, {from, scale = 1, duration, mode = EASE.out, level}) {
    pivotTowardsDock(card, dash);
    card.set({opacity: 0, scale_x: scale, scale_y: scale, ...towardsEdge(dash, from)});
    easeTo(card, {opacity: 255, scale_x: 1, scale_y: 1, translation_x: 0, translation_y: 0},
        {level, duration, mode});
}

// Goes back the same way, then is destroyed
export function popOut(card, dash, {to, scale = 1, duration, mode = EASE.out, level, onComplete}) {
    card.remove_all_transitions();
    card.reactive = false;
    easeTo(card, {opacity: 0, scale_x: scale, scale_y: scale, ...towardsEdge(dash, to)}, {
        level, duration, mode, onComplete: () => {
            card.destroy();
            onComplete?.();
        },
    });
}
