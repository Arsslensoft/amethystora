// What Desks shows: the card that asks the first time the computer is at a desk, and the toast that says what a known
// desk set, with Undo. Both are on the glass (the module lists them in glassSurfaces()), at the board's sizes
// (plans/beyond/boards/round4/Desks.dc.html); every word in them is text, never markup.

import Atk from 'gi://Atk';
import Clutter from 'gi://Clutter';
import St from 'gi://St';

import * as Main from 'resource:///org/gnome/shell/ui/main.js';

// The card 16 px in from the main screen's right edge and 10 px under the top bar; the toast 18 px above its bottom
const CARD_RIGHT = 16;
const CARD_GAP = 10;
const TOAST_BOTTOM = 18;
const FADE = 200;

function label(text, style) {
    const actor = new St.Label({style_class: style, text});
    actor.clutter_text.use_markup = false;
    return actor;
}

function scale() {
    return St.ThemeContext.get_for_stage(global.stage).scale_factor;
}

export function fadeIn(actor) {
    actor.opacity = 0;
    actor.ease({opacity: 255, duration: FADE, mode: Clutter.AnimationMode.EASE_OUT_QUAD});
}

// title, has: words; rows: [[name, value]]; buttons: [[words, primary, callback]]; note: words or null
export function makeCard({title, has, rows, buttons, note}) {
    const card = new St.BoxLayout({
        style_class: 'amethystora-desks-card',
        orientation: Clutter.Orientation.VERTICAL,
        reactive: true,
    });
    card.accessible_role = Atk.Role.NOTIFICATION;
    card.accessible_name = title;
    const head = new St.BoxLayout({style_class: 'amethystora-desks-head', x_expand: true});
    const name = label(title, 'amethystora-desks-title');
    name.x_expand = true;
    const what = label(has, 'amethystora-desks-has');
    what.y_align = Clutter.ActorAlign.END;
    head.add_child(name);
    head.add_child(what);
    card.add_child(head);
    for (const [key, value] of rows) {
        const row = new St.BoxLayout({style_class: 'amethystora-desks-row', x_expand: true});
        const left = label(key, 'amethystora-desks-row-name');
        left.x_expand = true;
        row.add_child(left);
        row.add_child(label(value, 'amethystora-desks-value'));
        card.add_child(row);
    }
    const line = new St.BoxLayout({style_class: 'amethystora-desks-buttons'});
    card.buttons = [];
    for (const [words, primary, callback] of buttons) {
        const button = new St.Button({
            style_class: `amethystora-desks-button${primary ? ' amethystora-desks-primary' : ''}`,
            label: words,
            can_focus: true,
        });
        button.connect('clicked', callback);
        line.add_child(button);
        card.buttons.push(button);
    }
    card.add_child(line);
    if (note) {
        const words = label(note, 'amethystora-desks-note');
        words.clutter_text.line_wrap = true;
        card.add_child(words);
    }
    Main.layoutManager.addTopChrome(card);
    const monitor = Main.layoutManager.primaryMonitor;
    const area = Main.layoutManager.getWorkAreaForMonitor(Main.layoutManager.primaryIndex);
    const [, width] = card.get_preferred_width(-1);
    card.set_position(Math.round(monitor.x + monitor.width - width - CARD_RIGHT * scale()),
        Math.round(area.y + CARD_GAP * scale()));
    fadeIn(card);
    return card;
}

export function makeToast(text, undoWords, onUndo) {
    const toast = new St.BoxLayout({style_class: 'amethystora-desks-toast', reactive: true});
    toast.accessible_role = Atk.Role.NOTIFICATION;
    toast.accessible_name = text;
    const words = label(text, 'amethystora-desks-toast-text');
    words.y_align = Clutter.ActorAlign.CENTER;
    toast.add_child(words);
    const undo = new St.Button({style_class: 'amethystora-desks-button amethystora-desks-undo', label: undoWords,
        can_focus: true});
    undo.connect('clicked', onUndo);
    toast.add_child(undo);
    toast.undo = undo;
    Main.layoutManager.addTopChrome(toast);
    // Above the dock where it keeps its room: from the bottom of the work area, again whenever that changes (a desk may
    // just have moved the dock)
    toast.place = () => {
        const area = Main.layoutManager.getWorkAreaForMonitor(Main.layoutManager.primaryIndex);
        const [, width] = toast.get_preferred_width(-1);
        const [, height] = toast.get_preferred_height(width);
        toast.set_position(Math.round(area.x + (area.width - width) / 2),
            Math.round(area.y + area.height - height - TOAST_BOTTOM * scale()));
    };
    global.display.connectObject('workareas-changed', () => toast.place(), toast);
    toast.connect('destroy', () => global.display.disconnectObject(toast));
    toast.place();
    fadeIn(toast);
    return toast;
}
