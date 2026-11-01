// How the dock's icons grow under the pointer (spec 1.1). Swell: every item grows with its distance from the pointer,
// a cosine wave reaching 2.34 pitches either side, the neighbours parting so that nothing overlaps and the row staying
// centred where it rests. Lift: only the item under the pointer rises, its two neighbours a little. Each item's
// picture (the tile inside its container) is scaled and moved; the containers, whose allocation is where the item
// rests, are never touched, so nothing is laid out again while the dock moves. The pill lengthens with the row.
//
// The values follow the pointer with a first-order lag (95 % in 90 ms inside, 120 ms on the way back to rest), on one
// timeline that runs only while some value is still moving: a still pointer over a settled dock costs nothing, and so
// does a pointer anywhere else, since the handlers listen to the dock's own events only.

import Clutter from 'gi://Clutter';
import GLib from 'gi://GLib';
import St from 'gi://St';

import {scaleFactor} from './util.js';

// The wave's reach in pitches: the board's 2.7 icons of 52 px at a 60 px pitch
const REACH = 2.34;
// Lift: the item under the pointer and its neighbours, at a 48 px icon
const LIFT = {scale: 1.22, away: 10, nextScale: 1.04, nextAway: 3};
// The lag's time constants in ms: 3 tau is the board's 90 ms inside and the plan's 120 ms back to rest
const TAU_IN = 30;
const TAU_OUT = 40;
// Below these a value counts as arrived
const SETTLE_SCALE = 0.002;
const SETTLE_PX = 0.25;

export class Swell {
    constructor(dash, dock) {
        this._dash = dash;
        this._dock = dock;
        this._items = [];
        this._stale = true;
        this._inside = false;
        this._pointer = null;
        this._running = false;
        this._mode = 'off';
        // The row is watched in every mode, Still too, so that its geometry at rest is never one read while it was
        // changing; only the pointer is not listened to while nothing grows
        this._row = {};
        dash._box.connectObject('notify::allocation', () => (this._stale = true),
            'child-added', () => (this._stale = true), 'child-removed', () => (this._stale = true), this._row);
    }

    // off, swell or lift; with the amount Swell grows to
    setMode(mode, amount) {
        this._amount = amount;
        if (mode === this._mode)
            return;
        const was = this._mode;
        this._mode = mode;
        if (mode === 'off') {
            // Back to rest first, then nothing is listened to
            this._pointer = null;
            this._retarget();
            if (!this._running)
                this._disconnect();
        } else if (was === 'off') {
            this._connect();
        }
    }

    // The values at rest at once, and nothing listened to (the dock goes, or is rebuilt)
    destroy() {
        this._disconnect();
        this._dash._box?.disconnectObject(this._row);
        this._timeline?.stop();
        this._timeline = null;
        for (const item of this._items)
            this._writeItem(item, 1, 0, 0);
        this._dash.pill?.set({scale_x: 1, scale_y: 1});
        this._setClip(true);
        this._items = [];
    }

    // The row changed: its geometry is read again before the next value
    invalidate() {
        this._stale = true;
    }

    // Something holds the dock at rest (a menu, a card, a drag) or lets it go
    sync() {
        this._retarget();
    }

    get running() {
        return this._running;
    }

    get items() {
        if (this._stale)
            this._measure();
        return this._items;
    }

    // ---- Events ----

    _connect() {
        if (this._connected)
            return;
        this._connected = true;
        this._dash.track_hover = true;
        this._dash.connectObject(
            'captured-event', (actor, event) => this._onEvent(event),
            'notify::hover', () => {
                if (this._dash.hover) {
                    this._stale = true;
                } else {
                    this._pointer = null;
                    this._retarget();
                }
            }, this);
    }

    _disconnect() {
        if (!this._connected)
            return;
        this._connected = false;
        this._dash.disconnectObject(this);
    }

    _onEvent(event) {
        if (event.type() !== Clutter.EventType.MOTION)
            return Clutter.EVENT_PROPAGATE;
        this._dock.motionCount++;
        this._pointer = event.get_coords();
        this._retarget();
        return Clutter.EVENT_PROPAGATE;
    }

    // ---- Geometry at rest ----

    // Every item of the row in order along it, with where it rests: from the containers, which the dock never
    // transforms, and the item's place inside its container
    _measure() {
        this._stale = false;
        const dash = this._dash;
        const {long} = dash.edge;
        const containers = dash._box.get_children().filter(child => child.visible && !child.animatingOut &&
            (child.child || child.has_style_class_name?.('dash-separator')));
        if (dash._showAppsIcon?.visible)
            containers.push(dash._showAppsIcon);
        const items = [];
        for (const container of containers) {
            // An item not laid out yet (GNOME has just added it): read again next time, since the row's own
            // allocation need not change when it is
            if (!container.has_allocation()) {
                this._stale = true;
                continue;
            }
            // An item growing in or shrinking away (GNOME eases its container): read again next time
            if (container.scale_x !== 1 || container.scale_y !== 1)
                this._stale = true;
            const extents = container.get_transformed_extents();
            const separator = !container.child;
            const actor = separator ? container : dash.visualOf(container);
            if (!actor)
                continue;
            const width = container.width || 1;
            const height = container.height || 1;
            const ratio = long === 'x' ? extents.size.width / width : extents.size.height / height;
            let start, len;
            if (separator) {
                start = long === 'x' ? extents.origin.x : extents.origin.y;
                len = long === 'x' ? extents.size.width : extents.size.height;
            } else {
                const box = actor.get_allocation_box();
                start = long === 'x' ? extents.origin.x + box.x1 * ratio : extents.origin.y + box.y1 * ratio;
                len = (long === 'x' ? box.get_width() : box.get_height()) * ratio;
            }
            if (len <= 0) {
                this._stale = true;
                continue;
            }
            const old = this._items.find(item => item.actor === actor);
            items.push({
                container, actor, separator,
                start, len, centre: start + len / 2,
                // The tile's size across the row, for the label's place
                across: long === 'x' ? actor.height : actor.width,
                shown: old?.shown ?? {scale: 1, along: 0, away: 0},
                target: old?.target ?? {scale: 1, along: 0, away: 0},
            });
        }
        items.sort((a, b) => a.centre - b.centre);
        // Items that left the row go back to rest
        for (const item of this._items) {
            if (!items.some(other => other.actor === item.actor))
                this._writeItem(item, 1, 0, 0);
        }
        this._items = items;
        for (const item of items) {
            this._pivot(item.actor);
            // An item that goes with the row is never written to again
            if (!item.actor._amethystoraWatched) {
                item.actor._amethystoraWatched = true;
                item.actor.connect('destroy', actor => (actor._amethystoraGone = true));
            }
        }

        const apps = items.filter(item => !item.separator);
        // The pitch: between two neighbours with no line between them
        this._pitch = 0;
        for (let i = 1; i < items.length; i++) {
            if (!items[i].separator && !items[i - 1].separator) {
                this._pitch = items[i].centre - items[i - 1].centre;
                break;
            }
        }
        if (!this._pitch)
            this._pitch = (dash.iconSize + 16) * scaleFactor();
        if (items.length) {
            const first = items[0];
            const last = items[items.length - 1];
            this._centre = (first.start + last.start + last.len) / 2;
            this._rowLength = last.start + last.len - first.start;
        }
        this._gaps = items.slice(1).map((item, i) => item.start - (items[i].start + items[i].len));
        const pill = dash.pill;
        this._pillLength = 0;
        if (pill)
            this._pillLength = long === 'x' ? pill.width : pill.height;
        this._dashRect = dash.restRect();
        this._appCount = apps.length;
    }

    _pivot(actor) {
        const edge = this._dash.edgeName;
        if (edge === 'left')
            actor.set_pivot_point(0, 0.5);
        else if (edge === 'right')
            actor.set_pivot_point(1, 0.5);
        else
            actor.set_pivot_point(0.5, 1);
    }

    // ---- Targets ----

    _retarget() {
        const items = this.items;
        if (!items.length)
            return;
        const level = this._dock.motionLevel();
        const held = this._mode === 'off' || level === 'reduced' || this._dash.held();
        let pointer = held ? null : this._pointer;
        if (pointer && this._dashRect) {
            const [x, y] = pointer;
            const r = this._dashRect;
            if (x < r.x || x > r.x + r.width || y < r.y || y > r.y + r.height)
                pointer = null;
        }
        this._inside = Boolean(pointer);
        const along = pointer ? pointer[this._dash.edge.long === 'x' ? 0 : 1] : 0;
        const per = this._dash.iconSize / 48 * scaleFactor();

        // The scale and the lift away from the edge of each item
        const scales = items.map(() => [1, 0]);
        if (pointer && this._mode === 'swell') {
            const reach = REACH * this._pitch;
            items.forEach((item, i) => {
                if (item.separator)
                    return;
                const d = Math.abs(along - item.centre);
                const f = d < reach ? (Math.cos(Math.PI * d / reach) + 1) / 2 : 0;
                scales[i][0] = 1 + (this._amount - 1) * f;
            });
        } else if (pointer && this._mode === 'lift') {
            let nearest = -1;
            let best = Infinity;
            items.forEach((item, i) => {
                const d = Math.abs(along - item.centre);
                if (!item.separator && d < best) {
                    best = d;
                    nearest = i;
                }
            });
            if (nearest >= 0 && best <= this._dash.iconSize * scaleFactor()) {
                scales[nearest] = [LIFT.scale, LIFT.away * per];
                const apps = items.map((item, i) => [item, i]).filter(([item]) => !item.separator);
                const at = apps.findIndex(([, i]) => i === nearest);
                for (const next of [apps[at - 1], apps[at + 1]]) {
                    if (next)
                        scales[next[1]] = [LIFT.nextScale, LIFT.nextAway * per];
                }
            }
        }

        // The row as it is drawn, centred where it rests
        const swollen = items.reduce((sum, item, i) => sum + item.len * scales[i][0], 0) +
            this._gaps.reduce((sum, gap) => sum + gap, 0);
        let edge = this._centre - swollen / 2;
        items.forEach((item, i) => {
            const len = item.len * scales[i][0];
            item.target = {scale: scales[i][0], along: edge + len / 2 - item.centre, away: scales[i][1]};
            edge += len + (this._gaps[i] ?? 0);
        });

        // Nothing to move to: the clock stays stopped
        if (!this._running && items.every(item => item.target.scale === item.shown.scale &&
            item.target.along === item.shown.along && item.target.away === item.shown.away)) {
            if (this._mode === 'off')
                this._disconnect();
            return;
        }
        if (level === 'none' || (level === 'reduced' && !this._running)) {
            for (const item of items)
                item.shown = {...item.target};
            this._write();
            return;
        }
        this._start();
    }

    // ---- The clock ----

    _start() {
        if (!this._timeline) {
            this._timeline = new Clutter.Timeline({actor: this._dash, duration: 1000, repeat_count: -1});
            this._timeline.connect('new-frame', () => this._frame());
        }
        if (this._running)
            return;
        this._running = true;
        this._last = GLib.get_monotonic_time();
        this._timeline.start();
    }

    _frame() {
        const began = GLib.get_monotonic_time();
        const dt = Math.min(100, Math.max(0, (began - this._last) / 1000));
        this._last = began;
        const slow = Math.max(0.01, St.Settings.get().slow_down_factor);
        const k = 1 - Math.exp(-dt / ((this._inside ? TAU_IN : TAU_OUT) * slow));
        let moving = false;
        for (const item of this._items) {
            const {shown, target} = item;
            for (const key of ['scale', 'along', 'away']) {
                shown[key] += (target[key] - shown[key]) * k;
                const close = key === 'scale' ? SETTLE_SCALE : SETTLE_PX;
                if (Math.abs(target[key] - shown[key]) > close)
                    moving = true;
            }
        }
        if (!moving) {
            for (const item of this._items)
                item.shown = {...item.target};
        }
        this._write();
        this._dock.noteFrame((GLib.get_monotonic_time() - began) / 1000);
        if (!moving) {
            this._timeline.stop();
            this._running = false;
            if (this._mode === 'off')
                this._disconnect();
        }
    }

    // ---- Drawing ----

    _write() {
        const {long} = this._dash.edge;
        let away = false;
        let swollen = 0;
        const changed = [];
        this._items.forEach(item => {
            const {scale, along, away: lift} = item.shown;
            // An item gone since the row was measured still counts: the row's length was measured with it, and a
            // pill worked out without it would stay one item short once the row is still
            swollen += item.len * scale;
            if (item.actor._amethystoraGone) {
                this._stale = true;
                return;
            }
            // Only what moved since the last frame is written: a property set costs more than the sum
            const last = item.written;
            if (item.actor?._amethystoraBusy) {
                item.written = null;
            } else if (!last || last.scale !== scale || last.along !== along || last.away !== lift) {
                this._writeItem(item, scale, along, lift);
                item.written = {scale, along, away: lift};
                changed.push(item);
            }
            if (scale !== 1 || along !== 0 || lift !== 0)
                away = true;
        });
        swollen += this._gaps.reduce((sum, gap) => sum + gap, 0);
        const pill = this._dash.pill;
        if (pill && this._rowLength) {
            const outside = this._pillLength - this._rowLength;
            const stretch = (swollen + outside) / (this._rowLength + outside);
            pill.set_pivot_point(0.5, 0.5);
            if (long === 'x')
                pill.scale_x = stretch;
            else
                pill.scale_y = stretch;
        }
        this._setClip(!away);
        if (changed.length)
            this._dash.followSwell(changed);
    }

    _writeItem(item, scale, along, lift) {
        const {actor} = item;
        if (!actor || actor._amethystoraBusy || actor._amethystoraGone)
            return;
        const edge = this._dash.edge;
        actor.set_scale(scale, scale);
        if (edge.long === 'x')
            actor.set_translation(along, edge.away * lift, 0);
        else
            actor.set_translation(edge.away * lift, along, 0);
    }

    // Something drawn beyond the row for a while (the numbers): the row is not cut meanwhile
    unclip(on) {
        this._unclipped = on;
        this._setClip(!on && this._items.every(item => item.shown.scale === 1 && item.shown.along === 0 &&
            item.shown.away === 0));
    }

    // The row is cut to its box while it can scroll; swollen icons are drawn beyond it
    _setClip(clip) {
        clip &&= !this._unclipped;
        if (this._clipped === clip)
            return;
        this._clipped = clip;
        const dash = this._dash;
        const row = dash._box?.get_parent();
        if (row)
            row.clip_to_view = clip;
        if (dash._scroll)
            dash._scroll.clip_to_allocation = clip;
        if (dash._box)
            dash._box.clip_to_allocation = clip;
    }
}
