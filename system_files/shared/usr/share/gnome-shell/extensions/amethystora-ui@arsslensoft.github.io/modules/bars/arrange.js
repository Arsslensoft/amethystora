// The main bar, arranged as the person wants it (the bars' `layout` key, ame desktop top-bar item, Control's Top bar):
// each item by its id in one of the bar's three boxes, in order, or hidden. An item is one or more of the panel's
// status-area roles: the tray is every tray icon, the tiling its three buttons, the system menu GNOME's quick settings
// with the screen-recording and sharing indicators and the privacy dots beside it, so that no arrangement can hide or
// part the dots. The clock and the system menu are never hidden. What no list names (another extension's button) keeps
// its place, toward the middle of the bar.
//
// Nothing here runs while nothing changes: the arrangement is applied when the key changes and when a box of the bar
// gains or loses a button, once before the next frame however many came. Moving a button that is already in place does
// nothing, so applying it again ends. Stopping puts every button it moved back in its box and place, and shows what it
// hid as its owner last wanted it.

import GLib from 'gi://GLib';
import Meta from 'gi://Meta';

import * as Main from 'resource:///org/gnome/shell/ui/main.js';

const LOGO = 'amethystora-logo-menu@arsslensoft.github.io';
// Each item's roles, in the order they sit together; a role ending in '*' stands for every role that begins so
export const ITEMS = {
    logo: [LOGO],
    workspaces: ['amethystora-workspaces'],
    tiling: ['amethystora-tiling-*'],
    clock: ['dateMenu'],
    processor: ['amethystora-meters-processor'],
    memory: ['amethystora-meters-memory'],
    network: ['amethystora-meters-network'],
    tray: ['amethystora-tray-*'],
    clipboard: ['amethystoraClipboard'],
    phone: ['amethystora-connect'],
    shield: ['amethystora-privacy-shield'],
    busy: ['amethystora-power-busy'],
    keyboard: ['keyboard'],
    a11y: ['a11y'],
    system: ['amethystora-privacy', 'screenRecording', 'screenSharing', 'dwellClick', 'quickSettings'],
};
// Never hidden: what tells the time and opens the system's own settings
export const FIXED = ['clock', 'system'];
export const ZONES = ['left', 'center', 'right'];
const WIDGET = /^widget-([a-z0-9][a-z0-9-]{0,39})$/;

// The roles an item's id stands for, or null for an id this image does not know
export function rolesOf(id) {
    if (ITEMS[id])
        return ITEMS[id];
    const widget = WIDGET.exec(id);
    return widget ? [`amethystora-widget-${widget[1]}`] : null;
}

// The layout as the key has it: the three boxes and hidden, each a list of known ids, an id in one list only (the
// first it is in), fixed items never hidden
export function cleanLayout(value) {
    const out = {left: [], center: [], right: [], hidden: []};
    const seen = new Set();
    for (const zone of [...ZONES, 'hidden']) {
        for (const id of Array.isArray(value?.[zone]) ? value[zone] : []) {
            if (typeof id !== 'string' || seen.has(id) || !rolesOf(id))
                continue;
            if (zone === 'hidden' && FIXED.includes(id))
                continue;
            seen.add(id);
            out[zone].push(id);
        }
    }
    return out;
}

export class Arrange {
    // changed(): the bar's buttons moved (the style marks them again, a bar at the bottom turns their menus)
    constructor(settings, changed) {
        this._settings = settings;
        this._changed = changed;
    }

    enable() {
        // The buttons moved since the bar was as it was (the snapshot), and for each one hidden whether its owner wants it shown
        this._origins = new Set();
        this._snapshot = null;
        // Each button watched for its end, and the owner of those watches
        this._ends = new Set();
        this._endOwner = {};
        this._hidden = new Map();
        this._watched = new Set();
        this._later = 0;
        this._settings.connectObject('changed::layout', () => this._queue(), this);
        this._watch();
        this._apply();
    }

    disable() {
        this._settings?.disconnectObject(this);
        this._settings = null;
        if (this._later)
            global.compositor.get_laters().remove(this._later);
        this._later = 0;
        for (const box of this._watched ?? [])
            box.disconnectObject(this);
        this._watched = null;
        this._restore();
        this._origins = null;
        this._hidden = null;
        this._ends = null;
    }

    // ---- For the tests ----

    // The ids in each box, left to right, and those hidden, as the bar has them now
    get placed() {
        const out = {left: [], center: [], right: [], hidden: []};
        const boxes = this._boxes();
        for (const [id] of Object.entries(ITEMS)) {
            for (const container of this._containers(id)) {
                const zone = this._hidden?.has(container) ? 'hidden'
                    : ZONES.find(z => container.get_parent() === boxes[z]);
                if (zone && !out[zone].includes(id))
                    out[zone].push(id);
            }
        }
        for (const zone of ZONES) {
            const order = boxes[zone].get_children();
            // By the last of each item's buttons, so that the system menu, whose indicators come first, is where its menu is
            const at = id => order.indexOf(this._containers(id).filter(c => c.get_parent() === boxes[zone]).at(-1));
            out[zone].sort((a, b) => at(a) - at(b));
        }
        return out;
    }

    // ---- Applying ----

    _boxes() {
        return {left: Main.panel._leftBox, center: Main.panel._centerBox, right: Main.panel._rightBox};
    }

    _layout() {
        try {
            return cleanLayout(this._settings.get_value('layout').recursiveUnpack());
        } catch {
            return cleanLayout(null);
        }
    }

    // The panel's containers an item has now, in the order of its roles
    _containers(id) {
        const area = Main.panel.statusArea;
        const out = [];
        for (const role of rolesOf(id) ?? []) {
            const names = role.endsWith('*')
                ? Object.keys(area).filter(name => name.startsWith(role.slice(0, -1))).sort(byNumber)
                : [role];
            for (const name of names) {
                const container = area[name]?.container;
                if (container && !out.includes(container))
                    out.push(container);
            }
        }
        return out;
    }

    _watch() {
        for (const box of Object.values(this._boxes())) {
            if (!box || this._watched.has(box))
                continue;
            box.connectObject(
                'child-added', () => this._queue(),
                'child-removed', () => this._queue(),
                this);
            this._watched.add(box);
        }
    }

    _queue() {
        if (this._later || !this._origins)
            return;
        this._later = global.compositor.get_laters().add(Meta.LaterType.BEFORE_REDRAW, () => {
            this._later = 0;
            try {
                this._apply();
            } catch (e) {
                console.error(`Amethystora UI: the top bar could not be arranged: ${e.message}`);
            }
            return GLib.SOURCE_REMOVE;
        });
    }

    _apply() {
        if (!this._origins)
            return;
        const layout = this._layout();
        const empty = [...ZONES, 'hidden'].every(zone => layout[zone].length === 0);
        if (empty) {
            this._restore();
            this._changed?.();
            return;
        }
        let moved = false;
        // Hidden first, so that a hidden item is never also placed
        const hide = new Set(layout.hidden.flatMap(id => this._containers(id)));
        for (const container of [...this._hidden.keys()]) {
            if (!hide.has(container))
                moved = this._show(container) || moved;
        }
        for (const container of hide)
            moved = this._hide(container) || moved;

        const boxes = this._boxes();
        for (const zone of ZONES) {
            const box = boxes[zone];
            const listed = layout[zone].flatMap(id => this._containers(id)).filter(c => !hide.has(c));
            for (const container of listed) {
                if (container.get_parent() === box)
                    continue;
                this._remember(container);
                container.get_parent()?.remove_child(container);
                box.add_child(container);
                moved = true;
            }
            const others = box.get_children().filter(c => !listed.includes(c));
            const order = zone === 'right' ? [...others, ...listed] : [...listed, ...others];
            order.forEach((container, index) => {
                if (box.get_child_at_index(index) !== container) {
                    this._remember(container);
                    box.set_child_at_index(container, index);
                    moved = true;
                }
            });
        }
        if (moved)
            this._changed?.();
    }

    // The bar as it was, taken before the first thing here moves or hides; and every button moved or hidden since
    _remember(container) {
        if (!this._snapshot) {
            this._snapshot = new Map(Object.values(this._boxes()).map(box => [box, box.get_children()]));
            for (const list of this._snapshot.values())
                list.forEach(c => this._watchEnd(c));
        }
        this._origins.add(container);
        this._watchEnd(container);
    }

    // A button destroyed (an app's tray icon gone) is forgotten everywhere
    _watchEnd(container) {
        if (this._ends.has(container))
            return;
        this._ends.add(container);
        container.connectObject('destroy', () => {
            this._ends?.delete(container);
            this._origins?.delete(container);
            this._hidden?.delete(container);
            for (const list of this._snapshot?.values() ?? []) {
                const at = list.indexOf(container);
                if (at >= 0)
                    list.splice(at, 1);
            }
        }, this._endOwner);
    }

    _hide(container) {
        if (this._hidden.has(container))
            return false;
        this._remember(container);
        this._hidden.set(container, container.visible);
        container.hide();
        // Its owner showing it again: kept hidden, and remembered as wanted
        container.connectObject('notify::visible', () => {
            if (!this._hidden?.has(container) || !container.visible)
                return;
            this._hidden.set(container, true);
            container.hide();
        }, this);
        return true;
    }

    _show(container) {
        if (!this._hidden.has(container))
            return false;
        const wanted = this._hidden.get(container);
        this._hidden.delete(container);
        container.disconnectObject(this);
        container.visible = wanted;
        return true;
    }

    // Everything back in the box and the order it was in before the first move. A button that came since keeps its
    // place after them (before them on the right, toward the middle), as it would beside any arrangement.
    _restore() {
        for (const container of [...this._hidden?.keys() ?? []])
            this._show(container);
        this._origins?.clear();
        const snapshot = this._snapshot;
        this._snapshot = null;
        if (snapshot) {
            const right = this._boxes().right;
            for (const [box, before] of snapshot) {
                const kept = before.filter(container => container.get_parent() !== null);
                for (const container of kept) {
                    if (container.get_parent() !== box) {
                        container.get_parent().remove_child(container);
                        box.add_child(container);
                    }
                }
                const others = box.get_children().filter(c => !kept.includes(c));
                const order = box === right ? [...others, ...kept] : [...kept, ...others];
                order.forEach((container, index) => {
                    if (box.get_child_at_index(index) !== container)
                        box.set_child_at_index(container, index);
                });
            }
        }
        for (const container of this._ends ?? [])
            container.disconnectObject(this._endOwner);
        this._ends?.clear();
    }
}

// Tray icons by the number GNOME's status area gave them, so that they keep the order they came in
function byNumber(a, b) {
    const n = s => Number(/(\d+)$/.exec(s)?.[1] ?? 0);
    return n(a) - n(b) || a.localeCompare(b);
}
