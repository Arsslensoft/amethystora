// Changing workspace with depth: GNOME slides the workspaces past, each one screen apart; here each workspace's
// wallpaper goes only a third as far as its windows, and the windows step back to 94 % on the way and come forward
// again, about the screen's centre. The switch takes a little longer than GNOME's and glides to rest. While motion is
// reduced the workspaces stay where they are and the new one fades in over the old one instead.
//
// Nothing of GNOME's is wrapped: GNOME builds one group per screen for a switch and moves it by its `progress`; a
// driver follows that property and moves only what is already in the group (the wallpapers' and the window copies'
// transforms), and GNOME throws the group away when the switch ends.

import Clutter from 'gi://Clutter';
import Graphene from 'gi://Graphene';
import Meta from 'gi://Meta';

import {adjustAnimationTime} from 'resource:///org/gnome/shell/misc/animationUtils.js';

import {FADE_MS, glide} from './path.js';

// The switch's time before GNOME's slow-down factor, and how far the windows step back
const DEPTH_MS = 560;
const DIP = 0.06;
// The wallpaper goes this share of the windows' way
const PARALLAX = 1 / 3;

const vertical = () => global.workspace_manager.layout_rows === -1;

export class Depth {
    // group: GNOME's MonitorGroup; mode: 'depth', or 'fade' while motion is reduced
    constructor(group, mode) {
        this.group = group;
        this.mode = mode;
        this._transition = null;
        this._laterId = global.compositor.get_laters().add(Meta.LaterType.BEFORE_REDRAW, () => {
            this._laterId = 0;
            this._retime();
            return false;
        });
        group.connectObject(
            'notify::progress', () => this._update(),
            'destroy', () => this.stop(false),
            this);
        this._update();
    }

    // GNOME has started its slide by now: it takes our time and our curve. A swipe has no slide until the fingers
    // let go, and keeps GNOME's ending then.
    _retime() {
        const t = this.group?.get_transition('progress');
        if (!t)
            return;
        if (this.mode === 'fade') {
            t.set_duration(Math.max(1, Math.round(adjustAnimationTime(FADE_MS))));
            t.set_progress_mode(Clutter.AnimationMode.EASE);
        } else {
            t.set_duration(Math.max(1, Math.round(adjustAnimationTime(DEPTH_MS))));
            t.set_progress_mode(Clutter.AnimationMode.CUBIC_BEZIER);
            t.set_cubic_bezier_progress(new Graphene.Point({x: 0.3, y: 0.7}), new Graphene.Point({x: 0.2, y: 1}));
        }
        this._transition = t;
    }

    _offset(wg, origin) {
        return vertical() ? wg.y + origin : wg.x + origin;
    }

    _update() {
        const group = this.group;
        const container = group?._container;
        const groups = group?._workspaceGroups;
        if (!container || !groups)
            return;
        const origin = vertical() ? container.y : container.x;
        if (this.mode === 'fade')
            this._fade(groups, origin);
        else
            this._depth(groups, origin);
    }

    // How far back the windows are now: by the time of the slide we retimed, else (a swipe) by how far it is
    // between two workspaces
    _scale() {
        const t = this._transition;
        if (t && this.group.get_transition('progress') === t && t.get_duration() > 0) {
            const share = Math.min(1, Math.max(0, t.get_elapsed_time() / t.get_duration()));
            return share < 0.5 ? 1 - DIP * glide(2 * share) : 1 - DIP + DIP * glide(2 * share - 1);
        }
        const p = this.group.progress;
        const points = (this.group.getSnapPoints?.() ?? []).slice().sort((a, b) => a - b);
        const below = points.filter(q => q <= p).pop();
        const above = points.find(q => q >= p);
        if (below === undefined || above === undefined || above - below < 1e-6)
            return 1;
        return 1 - DIP * Math.sin(Math.PI * (p - below) / (above - below));
    }

    _depth(groups, origin) {
        const s = this._scale();
        const monitor = this.group._monitor;
        const width = monitor?.width ?? this.group.width;
        const height = monitor?.height ?? this.group.height;
        const down = vertical();
        for (const wg of groups) {
            const offset = this._offset(wg, origin);
            if (wg._background) {
                if (down)
                    wg._background.translation_y = -(1 - PARALLAX) * offset;
                else
                    wg._background.translation_x = -(1 - PARALLAX) * offset;
            }
            // The screen's centre, in the workspace's own pixels
            const cx = down ? width / 2 : width / 2 - offset;
            const cy = down ? height / 2 - offset : height / 2;
            for (const record of wg._windowRecords ?? []) {
                const clone = record.clone;
                if (!clone || clone.width < 1 || clone.height < 1)
                    continue;
                clone.pivot_point = new Graphene.Point({
                    x: (cx - clone.x - clone.translation_x) / clone.width,
                    y: (cy - clone.y - clone.translation_y) / clone.height,
                });
                clone.set_scale(s, s);
            }
        }
    }

    // Nothing travels: every workspace is held on the screen, and of the two the switch is between, the one drawn
    // above shows as much as the switch has come towards it
    _fade(groups, origin) {
        const points = this.group.getSnapPoints?.() ?? [];
        const p = this.group.progress;
        const between = new Set();
        for (let i = 0; i < points.length; i++) {
            if (Math.abs(points[i] - p) < 1)
                between.add(i);
        }
        const children = this.group._container.get_children();
        const order = groups.map(wg => children.indexOf(wg));
        const ranked = [...between].sort((a, b) => order[b] - order[a]);
        const upper = ranked.length > 1 ? ranked[0] : -1;
        groups.forEach((wg, i) => {
            const offset = this._offset(wg, origin);
            if (vertical())
                wg.translation_y = -offset;
            else
                wg.translation_x = -offset;
            let opacity = 0;
            if (i === upper)
                opacity = Math.round(255 * Math.min(1, Math.max(0, 1 - Math.abs(p - points[i]))));
            else if (between.has(i))
                opacity = 255;
            wg.opacity = opacity;
        });
    }

    // Lets go of the group; `reset` puts back what was moved, for a group GNOME still shows
    stop(reset = true) {
        if (this._laterId)
            global.compositor.get_laters().remove(this._laterId);
        this._laterId = 0;
        const group = this.group;
        this.group = null;
        this._transition = null;
        if (!group)
            return;
        group.disconnectObject(this);
        if (!reset)
            return;
        for (const wg of group._workspaceGroups ?? []) {
            wg.translation_x = 0;
            wg.translation_y = 0;
            wg.opacity = 255;
            if (wg._background) {
                wg._background.translation_x = 0;
                wg._background.translation_y = 0;
            }
            for (const record of wg._windowRecords ?? [])
                record.clone?.set_scale(1, 1);
        }
    }
}
