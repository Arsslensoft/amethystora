// A plugin's card, drawn from the fixed set of elements (text, row, progress, button, switch, list, meter) and nothing else: no
// markup, no image from a path, no field to type in. Every text is a plain label. The plugin's name, what it may reach and its
// controls are drawn by the shell below what the plugin printed, whatever the plugin said, so that nothing in a card can pass for
// the system. Used by the pill's card (pill.js) and, as lines, by the top bar's menu.

import Clutter from 'gi://Clutter';
import Pango from 'gi://Pango';
import St from 'gi://St';

import {gettext as _} from 'resource:///org/gnome/shell/extensions/extension.js';

import {buttonRow, cardButton} from '../pill/clock.js';

export function label(text, styleClass = 'amethystora-pill-card-text') {
    const l = new St.Label({style_class: styleClass, text});
    l.clutter_text.use_markup = false;
    l.clutter_text.line_wrap = true;
    l.clutter_text.line_wrap_mode = Pango.WrapMode.WORD_CHAR;
    return l;
}

// A bar from 0 to 1: a track and a fill, both plain widgets
function bar(fraction) {
    const track = new St.Widget({style_class: 'amethystora-plugin-track', layout_manager: new Clutter.BinLayout(), x_expand: true});
    const fill = new St.Widget({style_class: 'amethystora-plugin-fill', x_align: Clutter.ActorAlign.START, x_expand: true, y_expand: true});
    track.add_child(fill);
    const set = f => {
        if (track.has_allocation())
            fill.width = Math.round(track.width * f);
    };
    track.connect('notify::width', () => set(fraction));
    return track;
}

function labelled(text, child) {
    const column = new St.BoxLayout({orientation: Clutter.Orientation.VERTICAL, style_class: 'amethystora-plugin-element'});
    if (text)
        column.add_child(label(text, 'amethystora-plugin-label'));
    column.add_child(child);
    return column;
}

// elements: the clamped card. run(action): what a button or a switch asks the host for
export function elements(card, run) {
    const body = new St.BoxLayout({orientation: Clutter.Orientation.VERTICAL, style_class: 'amethystora-plugin-elements'});
    for (const e of card) {
        switch (e.type) {
        case 'text':
            body.add_child(label(e.text));
            break;
        case 'row': {
            const row = new St.BoxLayout({style_class: 'amethystora-plugin-row'});
            const name = label(e.label, 'amethystora-plugin-label');
            name.x_expand = true;
            row.add_child(name);
            row.add_child(label(e.value, 'amethystora-plugin-value'));
            body.add_child(row);
            break;
        }
        case 'progress':
            body.add_child(labelled(e.label, bar(e.value)));
            break;
        case 'meter':
            body.add_child(labelled(`${e.label} ${Math.round(e.value * 100) / 100}`, bar((e.value - e.min) / (e.max - e.min))));
            break;
        case 'list': {
            const list = new St.BoxLayout({orientation: Clutter.Orientation.VERTICAL, style_class: 'amethystora-plugin-list'});
            for (const item of e.items)
                list.add_child(label(`• ${item}`));
            body.add_child(list);
            break;
        }
        case 'button':
            if (e.action)
                body.add_child(buttonRow([cardButton(e.label, () => run(e.action), {small: true})]));
            else
                body.add_child(label(e.label));
            break;
        case 'switch': {
            const toggle = cardButton(`${e.label}: ${e.on ? _('on') : _('off')}`, () => e.action && run(e.action),
                {small: true, primary: e.on, sensitive: Boolean(e.action)});
            body.add_child(buttonRow([toggle]));
            break;
        }
        }
    }
    return body;
}

// The words for what a plugin may reach, from its permissions' normal forms; nothing granted says so
export function reachWords(reach) {
    if (reach.length === 0)
        return _('It can reach nothing outside its own sandbox.');
    const words = reach.map(({kind, what}) => {
        switch (kind) {
        case 'network': return _('the network: %s').format(what);
        case 'network-local': return _('the home network: %s').format(what);
        case 'files': return _('your files: %s').format(what.replace(/:(ro|rw)$/, ''));
        case 'state': return _('what the computer says of %s').format(what);
        case 'control': return _('%s (after a click)').format(what);
        case 'events': return _('news of %s').format(what);
        case 'notify': return _('notifications');
        case 'open': return _('opening a web address (after a click)');
        case 'clipboard-write': return _('copying (after a click)');
        case 'pick': return _('one file you choose');
        case 'window': return _('a window of its own');
        case 'change': return _('settings: %s').format(what);
        default: return kind;
        }
    });
    return _('It can reach: %s.').format(words.join(', '));
}
