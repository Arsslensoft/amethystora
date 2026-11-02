// The live pill in the top bar. GNOME's own clock (its label, its time and its dot for unseen notifications) moves
// into the pill and stays exactly as it is while nothing is happening; while something is, the clock's place shows
// that thing on a round plate, and a click opens its card. A right-click, Super+V and the card's last row still open
// GNOME's calendar.
//
// The plate changes width with a slight overshoot, without a relayout per frame: the pill takes its new width at
// once, and the plate's three pieces (two round ends and the middle) start where the old outline was and ease to
// rest, by translation and scale only. Their contents cross-fade.
//
// Two at once (R7-16): a second activity sits beside the plate as a small bubble, a child of the pill, since the bar's
// button lays out only its first child. The pill takes the room for both at once, and each edge of the plate and the
// bubble travels from where it was, so that they split and rejoin in one motion. With one activity the two edges travel
// the same distance, as they always did.
//
// Weight (R7-17): that travel is a spring, carrying its speed across a change of mind (spring.js), one Clutter.Timeline
// while it moves; a pressed pill gives a little. Reduce motion has none of it, and the light desktop the old curve.

import Clutter from 'gi://Clutter';
import GLib from 'gi://GLib';
import GObject from 'gi://GObject';
import Graphene from 'gi://Graphene';
import Meta from 'gi://Meta';
import St from 'gi://St';

import {gettext as _} from 'resource:///org/gnome/shell/extensions/extension.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import * as PopupMenu from 'resource:///org/gnome/shell/ui/popupMenu.js';

import {RoundRing} from './ring.js';
import {SETTLE_MS, springAt} from './spring.js';
import {plateTint} from './tint.js';

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
// A pressed pill: how far it gives, and how long going in and coming back
const GIVE = 0.97;
const GIVE_IN = 70;
const GIVE_OUT = 240;
// The gentle glow when something ends, one rise or fall
const FLASH = 500;
// Between the plate and the bubble, and the most of a bubble's words
const GAP = 6;
const SHORT = 10;
// The plate's curve: a quick start and a slight overshoot before it rests
const CURVE = [new Graphene.Point({x: 0.2, y: 0.8}), new Graphene.Point({x: 0.2, y: 1.08})];

const PLATES = ['link', 'battery', 'saver', 'quiet', 'focus', 'news', 'hello', 'code', 'agent', 'agent-ask', 'agent-done'];
// The padding inside the plate at its start and its end (logical px): the link's and the battery's buttons sit close
// to the end
const PADDING = {link: [8, 6], battery: [12, 6]};

function scale() {
    return St.ThemeContext.get_for_stage(global.stage).scale_factor;
}

// An album's colour as a stylesheet says it, moved until the text reads on it, or nothing
function tintStyle(colour, text, alpha) {
    const tint = plateTint(colour, text);
    return tint ? `background-color: rgba(${tint[0]}, ${tint[1]}, ${tint[2]}, ${alpha});` : null;
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
        this.weight = false;
        this._bubble = null;
        this._ring = null;
        this._motion = null;
        this._laidBeside = 0;

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

    // The plate's width for what it shows now
    _plateWidth(forHeight = -1) {
        const shown = this._shown;
        if (!shown)
            return 0;
        const [, natural] = shown.get_preferred_width(forHeight);
        if (!this.active)
            return natural;
        const [start, end] = this._padding;
        return Math.min(natural + (start + end) * scale(), WIDEST * scale());
    }

    // The room the bubble takes beside the plate. One on its way out takes none: it is drawn over where it goes.
    _beside() {
        if (!this._bubble || this._bubble.leaving)
            return 0;
        return this._bubble.get_preferred_width(-1)[1] + GAP * scale();
    }

    // The width the pill takes for what it shows now
    _target(forHeight = -1) {
        return this._plateWidth(forHeight) + this._beside();
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
        const height = box.get_height();
        const plateHeight = Math.round(this.plateHeight);
        const top = Math.round((height - plateHeight) / 2);
        const cap = Math.round(plateHeight / 2);
        const rtl = this.get_text_direction() === Clutter.TextDirection.RTL;

        // The plate has what the bubble leaves; in a right-to-left language the bubble is at its left
        const whole = box.get_width();
        const beside = Math.min(this._beside(), whole);
        const width = whole - beside;
        const origin = rtl ? beside : 0;
        this._laidBeside = beside;
        this._plate.allocate(new Clutter.ActorBox({x1: origin, y1: 0, x2: origin + width, y2: height}));
        this._ring?.allocate(new Clutter.ActorBox({x1: 0, y1: top, x2: width, y2: top + plateHeight}));
        this._flash?.allocate(new Clutter.ActorBox({x1: 0, y1: top, x2: width, y2: top + plateHeight}));
        this._start.allocate(new Clutter.ActorBox({x1: 0, y1: top, x2: cap, y2: top + plateHeight}));
        this._middle.allocate(new Clutter.ActorBox({
            x1: cap, y1: top, x2: Math.max(cap, width - cap), y2: top + plateHeight,
        }));
        this._end.allocate(new Clutter.ActorBox({
            x1: Math.max(cap, width - cap), y1: top, x2: Math.max(2 * cap, width), y2: top + plateHeight,
        }));

        // The padding at the left and the right, which are the end and the start in a right-to-left language
        const [start, end] = this.active ? this._padding.map(p => p * s) : [0, 0];
        const [left, right] = rtl ? [end, start] : [start, end];
        const room = width - left - right;
        for (const child of this.get_children()) {
            if (child === this._plate)
                continue;
            if (child === this._bubble) {
                const w = child.get_preferred_width(-1)[1];
                const gap = GAP * s;
                const x = rtl ? (child.leaving ? -gap - w : 0) : width + gap;
                child.allocate(new Clutter.ActorBox({x1: x, y1: top, x2: x + w, y2: top + plateHeight}));
                continue;
            }
            const [, naturalWidth] = child.get_preferred_width(-1);
            // What is going keeps its own width while it fades; what shows fits the pill
            const w = child === this._shown ? Math.min(naturalWidth, Math.max(0, room)) : naturalWidth;
            const [, naturalHeight] = child.get_preferred_height(w);
            const h = child === this._clockBox ? height : Math.min(naturalHeight, height);
            const x = origin +
                (child === this._shown && this.active ? Math.round(left + (room - w) / 2) : Math.round((width - w) / 2));
            const y = Math.round((height - h) / 2);
            child.allocate(new Clutter.ActorBox({x1: x, y1: y, x2: x + w, y2: y + h}));
        }
    }

    // The outline on screen now, which a new change of width starts from
    _allocated() {
        return this.has_allocation() ? this.get_allocation_box().get_width() : 0;
    }

    // Where a plate sits in a pill of the whole width with this much beside it: its two edges, from the pill's middle,
    // which the bar keeps where it is
    _edgesAt(whole, beside) {
        const left = (this.get_text_direction() === Clutter.TextDirection.RTL ? beside : 0) - whole / 2;
        return [left, left + whole - beside];
    }

    // The outline on screen now, as its two edges. A change of width not laid out yet has drawn nothing: what shows is
    // still where that change started from (the pieces' translations are already the new width's)
    _outline() {
        const width = this._allocated();
        if (this._morphTo !== null && this._morphTo !== undefined && Math.abs(width - this._morphTo) >= 0.5)
            return this._morphFrom;
        const [left, right] = this._edgesAt(width, this._laidBeside);
        return [left + this._start.translation_x, right + this._end.translation_x];
    }

    // How fast the two edges move now, in pixels a second of the spring's own time: what a change of mind carries on with
    _speed() {
        const motion = this._motion;
        if (!motion)
            return [0, 0];
        const t = motion.elapsed / motion.scale;
        return [springAt(motion.left, motion.leftSpeed, t)[1], springAt(motion.right, motion.rightSpeed, t)[1]];
    }

    get morphing() {
        return !!this._motion ||
            this.pieces.some(piece => piece.get_transition('translation-x') || piece.get_transition('scale-x'));
    }

    // Where the bubble's left edge is now, from the pill's middle
    _bubbleAt() {
        const bubble = this._bubble;
        if (!bubble?.has_allocation())
            return null;
        return bubble.get_allocation_box().x1 - this._allocated() / 2 + bubble.translation_x;
    }

    // The bubble: {icon, text, progress, action}, or null. Called before the pill is told its new content, so that the
    // room for it is in the change of width that follows.
    setBubble(wanted, reduce) {
        const bubble = this._bubble?.leaving ? null : this._bubble;
        if (!wanted) {
            if (bubble)
                this._letBubbleGo(bubble, reduce);
            return;
        }
        if (bubble) {
            this._fillBubble(bubble, wanted);
            return;
        }
        // One still on its way out is gone at once: there is only one place beside the pill
        this._dropBubble();
        const made = new St.Widget({
            style_class: 'amethystora-pill-bubble',
            layout_manager: new Clutter.BinLayout(),
            opacity: 0,
        });
        made.box = new St.BoxLayout({style_class: 'amethystora-pill-bubble-box', y_align: Clutter.ActorAlign.CENTER});
        made.icon = new St.Icon({icon_size: 14, y_align: Clutter.ActorAlign.CENTER});
        made.label = new St.Label({y_align: Clutter.ActorAlign.CENTER});
        made.box.add_child(made.icon);
        made.box.add_child(made.label);
        made.add_child(made.box);
        made.arriving = true;
        this.add_child(made);
        this._bubble = made;
        this._fillBubble(made, wanted);
        made.ease({opacity: 255, duration: reduce ? REDUCED : IN, mode: Clutter.AnimationMode.EASE_OUT_QUAD});
    }

    _fillBubble(bubble, {icon, text, progress, action}) {
        bubble._pillAction = action;
        if (typeof icon === 'string') {
            if (bubble.icon.icon_name !== icon)
                bubble.icon.icon_name = icon;
        } else if (bubble.icon.gicon !== icon) {
            bubble.icon.gicon = icon;
        }
        const words = [...text ?? ''];
        const shown = words.length > SHORT ? words.slice(0, SHORT - 1).join('') + '…' : words.join('');
        if (bubble.label.text !== shown)
            bubble.label.text = shown;
        bubble.label.visible = shown !== '';
        if (Number.isFinite(progress)) {
            if (!bubble.ring) {
                bubble.ring = new RoundRing();
                bubble.add_child(bubble.ring);
            }
            bubble.ring.fraction = progress;
        } else if (bubble.ring) {
            bubble.ring.destroy();
            bubble.ring = null;
        }
    }

    _letBubbleGo(bubble, reduce) {
        bubble.leftAt = this._bubbleAt();
        bubble.leaving = true;
        bubble._pillAction = null;
        bubble.remove_all_transitions();
        bubble.ease({
            opacity: 0,
            duration: reduce ? REDUCED : OUT,
            mode: Clutter.AnimationMode.EASE_OUT_QUAD,
            onStopped: () => {
                if (this._bubble === bubble)
                    this._dropBubble();
            },
        });
    }

    _dropBubble() {
        const bubble = this._bubble;
        this._bubble = null;
        if (bubble) {
            bubble.remove_all_transitions();
            bubble.destroy();
        }
    }

    // The plate takes this colour (an album's), where the text reads on it, or keeps the theme's own
    setTint(colour) {
        const key = colour ? colour.join() : null;
        if (key === this._tinted)
            return;
        this._tinted = key;
        const style = tintStyle(colour, this._textColour(), 0.85);
        for (const piece of this.pieces)
            piece.set_style(style);
    }

    // The colour of what is shown, or white before it has one to ask for
    _textColour() {
        if (!this._shown?.get_stage())
            return [255, 255, 255];
        const color = this._shown.get_theme_node().get_foreground_color();
        return [color.red, color.green, color.blue];
    }

    // The ring round the plate: a fraction, or null for none
    setRing(fraction) {
        if (!Number.isFinite(fraction)) {
            this._ring?.destroy();
            this._ring = null;
            return;
        }
        if (!this._ring) {
            this._ring = new RoundRing();
            this._plate.add_child(this._ring);
        }
        this._ring.fraction = fraction;
    }

    // A gentle glow on the plate when something ends: twice up and down, or once and still under reduce motion
    flash(reduce) {
        this._flash?.destroy();
        const flash = new St.Widget({style_class: 'amethystora-pill-flash', opacity: 0});
        this._flash = flash;
        this._plate.add_child(flash);
        const gone = () => {
            if (this._flash === flash) {
                this._flash = null;
                flash.destroy();
            }
        };
        if (reduce) {
            flash.opacity = 200;
            flash.ease({opacity: 0, delay: FLASH, duration: FLASH, mode: Clutter.AnimationMode.LINEAR, onStopped: gone});
            return;
        }
        flash.ease({
            opacity: 255,
            duration: FLASH,
            repeatCount: 3,
            autoReverse: true,
            mode: Clutter.AnimationMode.EASE_IN_OUT_QUAD,
            onStopped: gone,
        });
    }

    // A pressed pill gives a little and comes back
    give() {
        this.set_pivot_point(0.5, 0.5);
        this.remove_transition('scale-x');
        this.remove_transition('scale-y');
        this.ease({
            scale_x: GIVE,
            scale_y: GIVE,
            duration: GIVE_IN,
            mode: Clutter.AnimationMode.EASE_OUT_QUAD,
            onComplete: () => this.ease({
                scale_x: 1,
                scale_y: 1,
                duration: GIVE_OUT,
                mode: Clutter.AnimationMode.EASE_OUT_BACK,
            }),
        });
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
        // Not laid out yet: what it would have carried on with still holds
        const speed = this._motion ? this._speed() : this._morphSpeed ?? [0, 0];
        this._stopMotion();
        this._morphFrom = from;
        this._morphTo = to;
        this._morphSpeed = speed;
        for (const piece of this.pieces) {
            piece.remove_transition('translation-x');
            piece.remove_transition('scale-x');
        }
        const beside = this._beside();
        const plate = to - beside;
        const [left, right] = this._edgesAt(to, beside);
        const [travelLeft, travelRight] = [from[0] - left, from[1] - right];
        const cap = this.plateHeight / 2;
        const bubbleTravel = this._bubbleTravel(from, to, left, right, reduce);
        const moves = !reduce && from[1] > from[0] && plate > 2 * cap &&
            (Math.abs(travelLeft) >= 0.5 || Math.abs(travelRight) >= 0.5);
        if (moves && this.weight && this.mapped) {
            this._spring(plate - 2 * cap, travelLeft, travelRight, speed, bubbleTravel);
        } else if (moves) {
            this._morphSpeed = null;
            const middle = plate - 2 * cap;
            const along = (travelLeft + travelRight) / 2;
            this._start.translation_x = travelLeft;
            this._end.translation_x = travelRight;
            this._middle.scale_x = Math.max(0, (middle + travelRight - travelLeft) / middle);
            this._middle.translation_x = along;
            easeCurved(this._start, {translation_x: 0}, MORPH);
            easeCurved(this._end, {translation_x: 0}, MORPH);
            easeCurved(this._middle, Math.abs(along) >= 0.5 ? {scale_x: 1, translation_x: 0} : {scale_x: 1}, MORPH);
            if (Math.abs(along) >= 0.5 && this._shown !== this._clockBox) {
                this._shown.translation_x = along;
                easeCurved(this._shown, {translation_x: 0}, MORPH);
            }
            if (this._bubble && !this._bubble.leaving && Math.abs(bubbleTravel) >= 0.5) {
                this._bubble.translation_x = bubbleTravel;
                easeCurved(this._bubble, {translation_x: 0}, MORPH);
            }
        } else {
            this._morphSpeed = null;
            this._rest();
        }
        // The ring comes back round the plate once it has nearly settled
        this._ring?.remove_transition('opacity');
        if (this._ring && moves) {
            this._ring.opacity = 0;
            this._ring.ease({opacity: 255, delay: MORPH, duration: IN, mode: Clutter.AnimationMode.EASE_OUT_QUAD});
        } else if (this._ring) {
            this._ring.opacity = 255;
        }
        if (Math.round(to) !== Math.round(this.has_allocation() ? this.get_allocation_box().get_width() : -1))
            this.queue_relayout();
    }

    // How far the bubble travels to its place: from where it was, or from inside the plate's end when it has only just
    // split off. One leaving slides into the plate's end as it fades, by itself.
    _bubbleTravel(from, to, left, right, reduce) {
        const bubble = this._bubble;
        if (!bubble)
            return 0;
        const width = bubble.get_preferred_width(-1)[1];
        const rtl = this.get_text_direction() === Clutter.TextDirection.RTL;
        const gap = GAP * scale();
        if (bubble.leaving) {
            const rest = rtl ? left - gap - width : right + gap;
            bubble.translation_x = (bubble.leftAt ?? rest) - rest;
            if (!reduce) {
                bubble.ease({
                    translation_x: rtl ? gap + width : -gap - width,
                    duration: MORPH,
                    mode: Clutter.AnimationMode.EASE_OUT_QUAD,
                });
            }
            return 0;
        }
        const rest = rtl ? -to / 2 : right + gap;
        const was = bubble.arriving ? (rtl ? from[0] : from[1] - width) : this._bubbleAt();
        bubble.arriving = false;
        return was === null ? 0 : was - rest;
    }

    // Everything at rest
    _rest() {
        this._start.translation_x = 0;
        this._end.translation_x = 0;
        this._middle.translation_x = 0;
        this._middle.scale_x = 1;
        if (this._shown && this._shown !== this._clockBox)
            this._shown.translation_x = 0;
        if (this._bubble && !this._bubble.leaving)
            this._bubble.translation_x = 0;
    }

    // One step of the spring: translation and scale only
    _step() {
        const motion = this._motion;
        const t = motion.elapsed / motion.scale;
        const [left] = springAt(motion.left, motion.leftSpeed, t);
        const [right] = springAt(motion.right, motion.rightSpeed, t);
        const along = (left + right) / 2;
        this._start.translation_x = left;
        this._end.translation_x = right;
        this._middle.translation_x = along;
        this._middle.scale_x = Math.max(0, (motion.middle + right - left) / motion.middle);
        if (this._shown && this._shown !== this._clockBox)
            this._shown.translation_x = along;
        if (motion.bubble && this._bubble && !this._bubble.leaving)
            this._bubble.translation_x = springAt(motion.bubble, 0, t)[0];
    }

    // GNOME's slow-down factor stretches the spring's own time: at 0.6 it is all over in 0.6 of the time
    _spring(middle, left, right, [leftSpeed, rightSpeed], bubble) {
        const factor = St.Settings.get().slow_down_factor || 1;
        const motion = {middle, left, right, bubble, leftSpeed, rightSpeed, scale: factor, elapsed: 0};
        this._motion = motion;
        motion.timeline = new Clutter.Timeline({actor: this, duration: Math.round(SETTLE_MS * factor)});
        motion.timeline.connect('new-frame', (timeline, ms) => {
            if (this._motion !== motion)
                return;
            motion.elapsed = ms / 1000;
            this._step();
        });
        motion.timeline.connect('completed', () => {
            if (this._motion !== motion)
                return;
            this._motion = null;
            this._morphSpeed = null;
            this._rest();
        });
        this._step();
        motion.timeline.start();
    }

    _stopMotion() {
        const motion = this._motion;
        this._motion = null;
        motion?.timeline.stop();
    }

    // Before the pill goes
    stopAll() {
        this._stopMotion();
        this._dropBubble();
        this.setRing(null);
        this._flash?.destroy();
        this._flash = null;
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
        actor.translation_x = 0;
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

    // Every actor of what is shown that a click acts on by itself (the chips and the bubble), at a point of the stage
    chipAt(x, y) {
        const bubble = this._bubble;
        if (bubble?._pillAction) {
            const extents = bubble.get_transformed_extents();
            const [x1, y1] = [extents.get_x(), extents.get_y()];
            if (x >= x1 && x <= x1 + extents.get_width() && y >= y1 && y <= y1 + extents.get_height())
                return bubble;
        }
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
    // twoAtOnce and weight: whether each is on now (R7-16, R7-17); left out, neither is
    constructor(activities, reduceMotion, {twoAtOnce = () => false, weight = () => false} = {}) {
        this._activities = activities;
        this._reduce = reduceMotion;
        this._twoAtOnce = twoAtOnce;
        this._weight = weight;
        this._card = null;
        this._cardId = null;
        this._showing = new Set();
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
        this._pill.connect('notify::mapped', () => this._tellShowing());
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
            for (const activity of this._showing)
                activity.showing?.(false);
            this._showing.clear();
            this._second = null;
            this._pill.stopAll();
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
        pill.weight = this._weight();
        let second = top && this._twoAtOnce() ? this._activities.second : null;
        // What a lower activity says must never keep the first from showing: a getter of its that throws leaves no bubble
        let bubble = null;
        if (second) {
            try {
                const lower = second;
                bubble = {
                    icon: lower.row.icon,
                    text: lower.short ?? '',
                    progress: lower.progress ?? null,
                    action: () => this._bubbleClicked(lower.id),
                };
            } catch (e) {
                console.warn(`Amethystora UI: the pill could not read ${second.id} for the bubble: ${e.message}`);
                second = null;
            }
        }
        this._second = second;
        pill.setTint(top?.tint ?? null);
        pill.setBubble(bubble, reduce);
        if (top) {
            pill.setPlate(top.plate ?? '');
            pill.setRing(top.progress ?? null);
            const content = top.pill();
            if (pill.shown !== content)
                pill.show(content, reduce);
            else
                pill.resize(reduce);
            // Said again when the activity changes and once a minute, not with every second a timer counts
            const now = GLib.get_monotonic_time();
            let said = top.accessible;
            try {
                if (second)
                    said = `${said}. ${second.accessible}`;
            } catch (e) {
                console.warn(`Amethystora UI: the pill could not read ${second.id} aloud: ${e.message}`);
            }
            const words = said.replace(/[0-9]/g, '');
            if (top !== this._named || words !== this._namedWords || now - this._namedAt >= 60e6) {
                this._named = top;
                this._namedWords = words;
                this._namedAt = now;
                this._dateMenu.accessible_name = said;
            }
        } else {
            this._named = null;
            pill.setRing(null);
            if (pill.shown !== this._box)
                pill.show(this._box, reduce);
            this._dateMenu.accessible_name = _('Clock and calendar');
        }
        // An activity that ended took its content away; one that is no longer first has faded out by itself
        for (const child of pill.get_children()) {
            if (child !== pill._plate && child !== pill._bubble && child !== this._box && child !== pill.shown &&
                child.opacity === 0 && !child.get_transition('opacity'))
                pill.release(child);
        }
        this._tellShowing();
        this._refreshCard();
    }

    // Tells the activities that want to know whether they can be seen: a timer's ring then moves, a meter reads
    _tellShowing() {
        const pill = this._pill;
        if (!pill)
            return;
        const now = new Set(pill.mapped ? [this._activities.top, this._second].filter(Boolean) : []);
        // One that throws must not stop the others, nor the card from being brought up to date after
        const tell = (activity, seen) => {
            try {
                activity.showing?.(seen);
            } catch (e) {
                console.warn(`Amethystora UI: ${activity.id} did not take being ${seen ? 'seen' : 'hidden'}: ${e.message}`);
            }
        };
        for (const activity of [...this._showing]) {
            if (!now.has(activity)) {
                this._showing.delete(activity);
                tell(activity, false);
            }
        }
        for (const activity of now) {
            if (!this._showing.has(activity)) {
                this._showing.add(activity);
                tell(activity, true);
            }
        }
    }

    // The bubble clicked: its own card, or what it does instead of one
    _bubbleClicked(id) {
        const activity = this._activities.get(id);
        if (activity?.activate) {
            this._closeCard();
            activity.activate();
        } else if (this._card && this._cardId === id) {
            this._closeCard();
        } else if (activity) {
            this.openCard(id);
        }
    }

    // The bubble's activity now, for the suites
    get bubbleId() {
        return this._second?.id ?? null;
    }

    // A gentle glow on the pill: something ended
    flash() {
        this._pill?.flash(this._reduce());
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
        if (this._weight() && !this._reduce())
            this._pill.give();
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
        // The card takes the album's colour too; another activity's card, the theme's
        box.set_style(tintStyle(shown.tint ?? null, this._pill._textColour(), 0.75));

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
