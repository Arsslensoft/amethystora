// The live pill in the top bar. GNOME's own clock (its label, its time and its dot for unseen notifications) moves
// into the pill and stays exactly as it is while nothing is happening; while something is, the clock's place shows
// that thing on a round plate, and a click opens its card. A right-click, Super+V and the card's last row still open
// GNOME's calendar.
//
// The plate changes width with a slight overshoot, without a relayout per frame: the pill takes its new width at
// once, and the plate's three pieces (two round ends and the middle) start where the old outline was and ease to
// rest, by translation and scale only. Their contents cross-fade.

import Clutter from 'gi://Clutter';
import GLib from 'gi://GLib';
import GObject from 'gi://GObject';
import Graphene from 'gi://Graphene';
import Meta from 'gi://Meta';
import St from 'gi://St';

import {gettext as _} from 'resource:///org/gnome/shell/extensions/extension.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import * as PopupMenu from 'resource:///org/gnome/shell/ui/popupMenu.js';

// Logical pixels: the plate's height (unless the bar's size gives another, -amethystora-plate-height in bars.css), the
// padding inside the plate, the widest it gets. A round end is half the plate's height wide.
const HEIGHT = 28;
const PAD = 12;
const WIDEST = 320;
// Milliseconds, before GNOME's slow-down factor: the plate's change of width, its fade between clear and tinted, the
// content going and coming, and the crossfade that replaces all of it with reduce motion
const MORPH = 360;
const PLATE_FADE = 300;
const OUT = 120;
const IN = 200;
const IN_DELAY = 140;
const REDUCED = 150;
// The plate's curve: a quick start and a slight overshoot before it rests
const CURVE = [new Graphene.Point({x: 0.2, y: 0.8}), new Graphene.Point({x: 0.2, y: 1.08})];

const PLATES = ['link', 'battery', 'saver', 'quiet', 'focus', 'news', 'agent', 'agent-ask', 'agent-done'];
// The padding inside the plate at its start and its end (logical px): the link's and the battery's buttons sit close
// to the end
const PADDING = {link: [8, 6], battery: [12, 6]};

function scale() {
    return St.ThemeContext.get_for_stage(global.stage).scale_factor;
}

// Eases an actor and gives the transitions it made the plate's curve
function easeCurved(actor, props, duration) {
    actor.ease({...props, duration, mode: Clutter.AnimationMode.EASE_OUT_QUAD});
    for (const name of Object.keys(props)) {
        const transition = actor.get_transition(name.replaceAll('_', '-'));
        transition?.set_cubic_bezier_progress(...CURVE);
    }
}

const PillActor = GObject.registerClass(
class AmethystoraPill extends St.Widget {
    _init() {
        super._init({
            style_class: 'amethystora-pill',
            y_expand: true,
            x_align: Clutter.ActorAlign.CENTER,
        });
        this.relayouts = 0;
        this._shown = null;
        this._clockBox = null;
        this._padding = [PAD, PAD];

        this._plate = new St.Widget({style_class: 'amethystora-pill-plate', opacity: 0});
        this._start = new St.Widget({style_class: 'amethystora-pill-piece amethystora-pill-piece-start'});
        this._middle = new St.Widget({
            style_class: 'amethystora-pill-piece amethystora-pill-piece-middle',
            pivot_point: new Graphene.Point({x: 0.5, y: 0.5}),
        });
        this._end = new St.Widget({style_class: 'amethystora-pill-piece amethystora-pill-piece-end'});
        this._plate.add_child(this._start);
        this._plate.add_child(this._middle);
        this._plate.add_child(this._end);
        this.add_child(this._plate);
    }

    get pieces() {
        return [this._start, this._middle, this._end];
    }

    // The plate's height in pixels: the bar's size can make it smaller or larger (modules/bars.js)
    get plateHeight() {
        try {
            const [found, length] = this.get_theme_node().lookup_length('-amethystora-plate-height', false);
            if (found && length > 0)
                return length;
        } catch {}
        return HEIGHT * scale();
    }

    get shown() {
        return this._shown;
    }

    get active() {
        return this._shown !== null && this._shown !== this._clockBox;
    }

    setClock(box) {
        this._clockBox = box;
        this.add_child(box);
        if (!this._shown)
            this._shown = box;
    }

    // The plate's own look: the dark tint, or the link's, the battery's, power saver's, quiet's or a focus profile's
    setPlate(kind) {
        this._padding = PADDING[kind] ?? [PAD, PAD];
        for (const name of PLATES) {
            if (name === kind)
                this._plate.add_style_class_name(`amethystora-pill-plate-${name}`);
            else
                this._plate.remove_style_class_name(`amethystora-pill-plate-${name}`);
        }
    }

    // The width the pill takes for what it shows now
    _target(forHeight = -1) {
        const shown = this._shown;
        if (!shown)
            return 0;
        const [, natural] = shown.get_preferred_width(forHeight);
        if (!this.active)
            return natural;
        const [start, end] = this._padding;
        return Math.min(natural + (start + end) * scale(), WIDEST * scale());
    }

    vfunc_get_preferred_width(forHeight) {
        const width = this._target(forHeight);
        return [width, width];
    }

    vfunc_get_preferred_height(forWidth) {
        let height = this.plateHeight;
        for (const child of this.get_children()) {
            if (child === this._plate || !child.visible)
                continue;
            const [, natural] = child.get_preferred_height(-1);
            height = Math.max(height, natural);
        }
        return [height, height];
    }

    vfunc_allocate(box) {
        this.set_allocation(box);
        this.relayouts++;
        const s = scale();
        const width = box.get_width();
        const height = box.get_height();
        const plateHeight = Math.round(this.plateHeight);
        const top = Math.round((height - plateHeight) / 2);
        const cap = Math.round(plateHeight / 2);

        this._plate.allocate(new Clutter.ActorBox({x1: 0, y1: 0, x2: width, y2: height}));
        this._start.allocate(new Clutter.ActorBox({x1: 0, y1: top, x2: cap, y2: top + plateHeight}));
        this._middle.allocate(new Clutter.ActorBox({
            x1: cap, y1: top, x2: Math.max(cap, width - cap), y2: top + plateHeight,
        }));
        this._end.allocate(new Clutter.ActorBox({
            x1: Math.max(cap, width - cap), y1: top, x2: Math.max(2 * cap, width), y2: top + plateHeight,
        }));

        // The padding at the left and the right, which are the end and the start in a right-to-left language
        const rtl = this.get_text_direction() === Clutter.TextDirection.RTL;
        const [start, end] = this.active ? this._padding.map(p => p * s) : [0, 0];
        const [left, right] = rtl ? [end, start] : [start, end];
        const room = width - left - right;
        for (const child of this.get_children()) {
            if (child === this._plate)
                continue;
            const [, naturalWidth] = child.get_preferred_width(-1);
            // What is going keeps its own width while it fades; what shows fits the pill
            const w = child === this._shown ? Math.min(naturalWidth, Math.max(0, room)) : naturalWidth;
            const [, naturalHeight] = child.get_preferred_height(w);
            const h = child === this._clockBox ? height : Math.min(naturalHeight, height);
            const x = child === this._shown && this.active ? Math.round(left + (room - w) / 2) : Math.round((width - w) / 2);
            const y = Math.round((height - h) / 2);
            child.allocate(new Clutter.ActorBox({x1: x, y1: y, x2: x + w, y2: y + h}));
        }
    }

    // The outline on screen now, which a new change of width starts from
    _allocated() {
        return this.has_allocation() ? this.get_allocation_box().get_width() : 0;
    }

    // The outline on screen now. A change of width not laid out yet has drawn nothing: what shows is still where that
    // change started from (the pieces' translations are already the new width's)
    _outline() {
        const width = this._allocated();
        if (this._morphTo !== null && this._morphTo !== undefined && Math.abs(width - this._morphTo) >= 0.5)
            return this._morphFrom;
        return width - 2 * this._start.translation_x;
    }

    get morphing() {
        return this.pieces.some(piece => piece.get_transition('translation-x') || piece.get_transition('scale-x'));
    }

    // Shows `actor` (an activity's content, or the clock's own box) in place of what is shown now
    show(actor, reduce) {
        const before = this._shown;
        const from = this._outline();
        this._morphTo = null;
        this._shown = actor;
        if (actor !== before) {
            if (actor.get_parent() !== this)
                this.add_child(actor);
            this._fadeOut(before, reduce);
            this._fadeIn(actor, reduce);
            this._plate.remove_transition('opacity');
            this._plate.ease({
                opacity: this.active ? 255 : 0,
                duration: reduce ? REDUCED : PLATE_FADE,
                mode: Clutter.AnimationMode.EASE,
            });
        }
        this.resize(reduce, from);
    }

    // The width of what is shown may have changed: the plate follows
    resize(reduce, from = null) {
        const to = this._target();
        // A change toward this width runs already, or waits for its layout: it goes on as it is (an activity says
        // several things in a row as it starts, each of which asks again)
        if (from === null && Math.abs(to - (this._morphTo ?? -1)) < 0.5 &&
            (this.morphing || Math.abs(this._allocated() - to) >= 0.5))
            return;
        from ??= this._outline();
        this._morphFrom = from;
        this._morphTo = to;
        for (const piece of this.pieces) {
            piece.remove_transition('translation-x');
            piece.remove_transition('scale-x');
        }
        const cap = this.plateHeight / 2;
        const travel = (to - from) / 2;
        if (!reduce && from > 0 && to > 2 * cap && Math.abs(travel) >= 0.5) {
            this._start.translation_x = travel;
            this._end.translation_x = -travel;
            this._middle.scale_x = Math.max(0, (from - 2 * cap) / (to - 2 * cap));
            easeCurved(this._start, {translation_x: 0}, MORPH);
            easeCurved(this._end, {translation_x: 0}, MORPH);
            easeCurved(this._middle, {scale_x: 1}, MORPH);
        } else {
            this._start.translation_x = 0;
            this._end.translation_x = 0;
            this._middle.scale_x = 1;
        }
        if (Math.round(to) !== Math.round(this.has_allocation() ? this.get_allocation_box().get_width() : -1))
            this.queue_relayout();
    }

    _fadeOut(actor, reduce) {
        if (!actor || actor.get_parent() !== this)
            return;
        actor.remove_transition('opacity');
        // An activity may destroy its content before the fade ends: a disposed actor is not touched
        let gone = false;
        const tag = {};
        actor.connectObject('destroy', () => { gone = true; }, tag);
        actor.ease({
            opacity: 0,
            duration: reduce ? REDUCED : OUT,
            mode: Clutter.AnimationMode.EASE_OUT_QUAD,
            onStopped: () => {
                if (gone)
                    return;
                actor.disconnectObject(tag);
                if (this._shown === actor || actor.get_parent() !== this)
                    return;
                // GNOME's clock box stays a child of the pill; an activity's content goes back to its activity
                if (actor === this._clockBox)
                    actor.hide();
                else
                    this.remove_child(actor);
            },
        });
    }

    _fadeIn(actor, reduce) {
        actor.remove_transition('opacity');
        actor.show();
        actor.opacity = 0;
        actor.ease({
            opacity: 255,
            delay: reduce ? 0 : IN_DELAY,
            duration: reduce ? REDUCED : IN,
            mode: Clutter.AnimationMode.EASE_OUT_QUAD,
        });
    }

    // Lets go of an activity's content without destroying it (the activity owns it)
    release(actor) {
        if (actor?.get_parent() === this) {
            // Stopping a transition runs its completion, which may take the actor out of the pill or destroy it
            let gone = false;
            const tag = {};
            actor.connectObject('destroy', () => { gone = true; }, tag);
            actor.remove_all_transitions();
            if (!gone) {
                actor.disconnectObject(tag);
                if (actor.get_parent() === this)
                    this.remove_child(actor);
            }
        }
        if (this._shown === actor)
            this._shown = this._clockBox;
    }

    // Every actor of what is shown that a click acts on by itself (the chips), at a point of the stage
    chipAt(x, y) {
        const visit = actor => {
            if (!actor.visible)
                return null;
            if (actor._pillAction) {
                const extents = actor.get_transformed_extents();
                const [x1, y1] = [extents.get_x(), extents.get_y()];
                if (x >= x1 && x <= x1 + extents.get_width() && y >= y1 && y <= y1 + extents.get_height())
                    return actor;
            }
            for (const child of actor.get_children()) {
                const found = visit(child);
                if (found)
                    return found;
            }
            return null;
        };
        return this.active ? visit(this._shown) : null;
    }
});

// Keys pressed on an actor: GNOME 51 hands them to an action (Clutter.KeyController), 50 to a signal. `handler` gets
// the key's symbol and says whether it took it.
export function onKey(actor, handler) {
    if (Clutter.KeyController) {
        const controller = new Clutter.KeyController();
        controller.connect('key-press', () => {
            const [, symbol] = controller.get_key();
            return handler(symbol) ? Clutter.EVENT_STOP : Clutter.EVENT_PROPAGATE;
        });
        actor.add_action(controller);
        return () => actor.remove_action(controller);
    }
    const id = actor.connect('key-press-event',
        (a, event) => (handler(event.get_key_symbol()) ? Clutter.EVENT_STOP : Clutter.EVENT_PROPAGATE));
    return () => actor.disconnect(id);
}

// An activity's content that has ended: destroyed once the pill has let it go (after its fade), or at once
export function retire(actor) {
    if (!actor)
        return;
    if (!actor.get_parent()) {
        actor.destroy();
        return;
    }
    actor.connect('parent-set', () => {
        if (!actor.get_parent())
            actor.destroy();
    });
}

// A button of a card: the primary one in the accent's colours
export function cardButton(label, action, {primary = false, sensitive = true, small = false} = {}) {
    const button = new St.Button({
        style_class: `amethystora-pill-button${small ? ' amethystora-pill-button-small' : ''}` +
            `${primary ? ' amethystora-pill-button-primary' : ''}`,
        label,
        can_focus: true,
        reactive: sensitive,
        accessible_name: label,
    });
    button.clutter_text?.set_use_markup?.(false);
    if (!sensitive)
        button.add_style_pseudo_class('insensitive');
    button.connect('clicked', () => action());
    return button;
}

// A row of card buttons
export function buttonRow(buttons, {center = false} = {}) {
    const row = new St.BoxLayout({
        style_class: 'amethystora-pill-card-buttons',
        x_align: center ? Clutter.ActorAlign.CENTER : Clutter.ActorAlign.START,
    });
    for (const button of buttons)
        row.add_child(button);
    return row;
}

// A chip in the pill: its action runs from the pill's own click, which finds it by position
export function chip(label, action) {
    const actor = new St.Label({style_class: 'amethystora-pill-chip', text: label, y_align: Clutter.ActorAlign.CENTER});
    actor.clutter_text.y_align = Clutter.ActorAlign.CENTER;
    actor._pillAction = action;
    return actor;
}

// A card's menu, destroyed once its closing has finished and the frame after it is drawn: GNOME's box pointer still
// sets its own properties after it hides, and a menu destroyed before the frame is laid out again after it is gone
function destroyWhenShut(menu) {
    const destroy = () => global.compositor.get_laters().add(Meta.LaterType.IDLE, () => {
        Main.panel.menuManager.removeMenu(menu);
        menu.destroy();
        return GLib.SOURCE_REMOVE;
    });
    if (menu.actor.visible)
        menu.actor.connect('hide', () => destroy());
    else
        destroy();
}

export class Clock {
    constructor(activities, reduceMotion) {
        this._activities = activities;
        this._reduce = reduceMotion;
        this._card = null;
        this._cardId = null;
    }

    enable() {
        const dateMenu = Main.panel.statusArea.dateMenu;
        const box = dateMenu?._clockDisplay?.get_parent();
        if (!dateMenu || !box || box.get_parent() !== dateMenu)
            throw new Error('GNOME\'s clock is not where the pill looks for it');
        this._dateMenu = dateMenu;
        this._box = box;
        this._name = dateMenu.accessible_name;

        this._pill = new PillActor();
        dateMenu.remove_child(box);
        this._pill.setClock(box);
        dateMenu.insert_child_at_index(this._pill, 0);

        // GNOME's own click opens the calendar on any button; the pill's clicks take its place while it runs
        dateMenu._clickGesture?.set_enabled(false);
        this._gestures = [
            [Clutter.BUTTON_PRIMARY, gesture => this._primary(gesture)],
            [Clutter.BUTTON_SECONDARY, () => this._calendar()],
            [Clutter.BUTTON_MIDDLE, () => this._calendar()],
        ].map(([button, handler]) => {
            const gesture = new Clutter.ClickGesture({required_button: button, recognize_on_press: true});
            gesture.connect('recognize', handler);
            dateMenu.add_action(gesture);
            return gesture;
        });

        // Return, Enter and space do what a click on the pill's body does
        this._unkey = onKey(dateMenu, symbol => {
            if (![Clutter.KEY_Return, Clutter.KEY_KP_Enter, Clutter.KEY_space].includes(symbol))
                return false;
            this._body();
            return true;
        });
        this.sync();
    }

    disable() {
        this._closeCard();
        const dateMenu = this._dateMenu;
        if (!dateMenu)
            return;
        for (const gesture of this._gestures ?? [])
            dateMenu.remove_action(gesture);
        this._gestures = null;
        this._unkey?.();
        this._unkey = null;

        if (this._pill) {
            // The activities keep their contents; GNOME gets its clock back as it was
            for (const child of this._pill.get_children()) {
                if (child !== this._box && child !== this._pill._plate)
                    this._pill.release(child);
            }
            this._box.remove_all_transitions();
            this._pill.remove_child(this._box);
            this._pill.destroy();
            this._pill = null;
        }
        if (this._box.get_parent() === null)
            dateMenu.insert_child_at_index(this._box, 0);
        this._box.opacity = 255;
        this._box.show();
        dateMenu._clickGesture?.set_enabled(dateMenu.menu !== null);
        dateMenu.accessible_name = this._name ?? '';
        this._dateMenu = null;
    }

    get actor() {
        return this._pill;
    }

    get relayouts() {
        return this._pill?.relayouts ?? 0;
    }

    get morphing() {
        return this._pill?.morphing ?? false;
    }

    get cardId() {
        return this._card ? this._cardId : null;
    }

    // What the activities say now, in the pill: a new first activity, or a change of what the first one shows
    sync() {
        const pill = this._pill;
        if (!pill)
            return;
        const top = this._activities.top;
        const reduce = this._reduce();
        if (top) {
            pill.setPlate(top.plate ?? '');
            const content = top.pill();
            if (pill.shown !== content)
                pill.show(content, reduce);
            else
                pill.resize(reduce);
            // Said again when the activity changes and once a minute, not with every second a timer counts
            const now = GLib.get_monotonic_time();
            const words = top.accessible.replace(/[0-9]/g, '');
            if (top !== this._named || words !== this._namedWords || now - this._namedAt >= 60e6) {
                this._named = top;
                this._namedWords = words;
                this._namedAt = now;
                this._dateMenu.accessible_name = top.accessible;
            }
        } else {
            this._named = null;
            if (pill.shown !== this._box)
                pill.show(this._box, reduce);
            this._dateMenu.accessible_name = _('Clock and calendar');
        }
        // An activity that ended took its content away; one that is no longer first has faded out by itself
        for (const child of pill.get_children()) {
            if (child !== pill._plate && child !== this._box && child !== pill.shown && child.opacity === 0 &&
                !child.get_transition('opacity'))
                pill.release(child);
        }
        this._refreshCard();
    }

    // An activity is ending: its content must leave the pill before the activity destroys it
    forget(actor) {
        this._pill?.release(actor);
    }

    _primary(gesture) {
        if (!this._activities.top) {
            this._calendar();
            return;
        }
        let x = -1, y = -1;
        const point = gesture.get_coords_abs?.();
        if (Array.isArray(point))
            [x, y] = point;
        else if (point)
            [x, y] = [point.x, point.y];
        const chipActor = this._pill.chipAt(x, y);
        if (chipActor) {
            chipActor._pillAction();
            return;
        }
        this._body();
    }

    // A click on the pill's body, or Return on it
    _body() {
        const top = this._activities.top;
        if (!top) {
            this._calendar();
        } else if (top.activate) {
            this._closeCard();
            top.activate();
        } else if (this._card) {
            this._closeCard();
        } else {
            this.openCard();
        }
    }

    _calendar() {
        this._closeCard();
        this._dateMenu?.menu.toggle();
    }

    // The card of the first activity, or of the activity `id`
    openCard(id = null) {
        if (!this._dateMenu || !this._activities.top)
            return;
        this._dateMenu.menu.close();
        if (!this._card) {
            const menu = new PopupMenu.PopupMenu(this._dateMenu, 0.5, St.Side.TOP);
            menu.actor.add_style_class_name('amethystora-pill-pointer');
            menu.box.add_style_class_name('amethystora-pill-card');
            Main.uiGroup.add_child(menu.actor);
            menu.actor.hide();
            Main.panel.menuManager.addMenu(menu);
            menu.connect('open-state-changed', (m, open) => {
                if (!open)
                    this._cardClosed(m);
            });
            this._card = menu;
        }
        this._cardId = id ?? this._activities.top.id;
        this._fillCard();
        if (!this._card.isOpen)
            this._card.toggle();
    }

    _cardClosed(menu) {
        if (this._card !== menu)
            return;
        this._card = null;
        this._cardId = null;
        destroyWhenShut(menu);
    }

    _closeCard() {
        const menu = this._card;
        if (!menu)
            return;
        this._card = null;
        this._cardId = null;
        menu.close();
        destroyWhenShut(menu);
    }

    // The card follows the activities while it is open
    _refreshCard() {
        if (!this._card)
            return;
        const list = this._activities.list;
        if (list.length === 0) {
            this._closeCard();
            return;
        }
        const ids = list.map(a => a.id).join();
        if (!list.some(a => a.id === this._cardId))
            this._cardId = list[0].id;
        if (ids !== this._cardIds)
            this._fillCard();
    }

    _fillCard() {
        const box = this._card.box;
        box.destroy_all_children();
        const list = this._activities.list;
        this._cardIds = list.map(a => a.id).join();
        const shown = list.find(a => a.id === this._cardId) ?? list[0];
        const api = {close: () => this._closeCard(), calendar: () => this._calendar()};
        box.add_child(shown.card(api));

        const others = list.filter(a => a !== shown).slice(0, 3);
        if (others.length > 0) {
            box.add_child(new St.Widget({style_class: 'amethystora-pill-card-line'}));
            for (const other of others) {
                const row = new St.Button({
                    style_class: 'amethystora-pill-card-row',
                    can_focus: true,
                    x_expand: true,
                    accessible_name: other.row.text,
                });
                const content = new St.BoxLayout({style_class: 'amethystora-pill-card-row-box', x_expand: true, x_align: Clutter.ActorAlign.START});
                content.add_child(new St.Icon({
                    style_class: 'amethystora-pill-card-row-icon',
                    ...(typeof other.row.icon === 'string' ? {icon_name: other.row.icon} : {gicon: other.row.icon}),
                    icon_size: 16,
                }));
                content.add_child(new St.Label({text: other.row.text, y_align: Clutter.ActorAlign.CENTER}));
                row.set_child(content);
                row.connect('clicked', () => {
                    this._cardId = other.id;
                    this._fillCard();
                });
                box.add_child(row);
            }
        }

        // While an activity shows, the bar has no time: the card's last row has it, and opens the calendar
        const clockRow = new St.Button({
            style_class: 'amethystora-pill-card-clock',
            label: this._dateMenu._clockDisplay?.text ?? '',
            can_focus: true,
            x_align: Clutter.ActorAlign.CENTER,
        });
        clockRow.accessible_name = _('Open the calendar');
        clockRow.connect('clicked', () => this._calendar());
        box.add_child(clockRow);
    }
}
