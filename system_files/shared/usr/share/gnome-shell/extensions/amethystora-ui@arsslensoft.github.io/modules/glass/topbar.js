// The top bar, as the desktop style makes it (modules/style.js). Where the bar is one pane (floating, neon) it is a
// clear strip of frosted wallpaper with the bar's corners, darkened a little so that its white text reads on a pale
// one, until a window on the bar's screen and workspace reaches up to within 5 pixels of it; then it fades to the
// bar's own background over half a second, and back once no window does. Where the bar is a row of islands, each
// button is a pane of its own. A solid bar, and one that is only its buttons, have no glass. In the overview it is
// clear, and the panes are gone from the moment the overview starts to open until it has closed. With the tiling's
// gap of 8 pixels under the bar, tiled windows leave it clear. While the glass is solid the bar is never clear: it
// shows the style's own background, as when a window touches it.
// The other screens' bars (modules/bars.js) are each treated as the main one, with the windows of their own screen.
// Where the style leaves the bar bare over the wallpaper with text straight on it (capsule), the bar has only the veil
// its text needs over what lies right behind it, and nothing at all over a wallpaper dark enough. The glass keeps the
// text readable where it can: on the bar's pane the clock's own colour (the neon style draws it in the accent), on a
// pill the pill's own label, under whatever background the style gives the pill.

import Meta from 'gi://Meta';
import St from 'gi://St';

import * as Main from 'resource:///org/gnome/shell/ui/main.js';

import {TYPES, leftAlone} from './windows.js';

// How close a window's top may come to the bar's bottom before the bar takes its background back, in pixels
const NEAR = 5;
const CLEAR = 'amethystora-glass-clear';
// The clock is the text the bar's glass keeps readable
const text = () => Main.panel.statusArea.dateMenu?._clockDisplay ?? Main.panel.statusArea.dateMenu ?? Main.panel;
// The first label inside an actor, whose colour is the text a pill or a bare bar shows
const firstLabel = actor => {
    if (actor instanceof St.Label)
        return actor;
    for (const child of actor.get_children()) {
        const found = firstLabel(child);
        if (found)
            return found;
    }
    return null;
};
const pillText = button => firstLabel(button) ?? button;
const bareText = () => firstLabel(Main.panel._rightBox) ?? Main.panel;
// The styles whose bare bar carries text straight over the wallpaper
const BARE_TEXT = new Set(['capsule']);

export class TopBar {
    constructor(glass) {
        this._glass = glass;
    }

    enable() {
        this._laterId = 0;
        this._ended = false;
        // The bars with a pane: the main one, and the other screens' (modules/bars.js)
        this._bars = new Set();
        this._pills = new Set();
        this._inOverview = Main.overview.visible;
        Main.overview.connectObject(
            'showing', () => {
                this._inOverview = true;
                this._sync();
            },
            'hidden', () => {
                this._inOverview = false;
                this._sync();
            }, this);
        Main.sessionMode.connectObject('updated', () => this._queue(), this);
        // GNOME Shell ending destroys its bar without turning the extensions off first: nothing touches it after
        Main.panel.connectObject('destroy', () => {
            this._ended = true;
            if (this._laterId)
                global.compositor.get_laters().remove(this._laterId);
            this._laterId = 0;
        }, this);
        this._glass.windows?.onChange(() => this._queue());
        this.restyle();
    }

    disable() {
        Main.overview.disconnectObject(this);
        Main.sessionMode.disconnectObject(this);
        if (!this._ended)
            Main.panel.disconnectObject(this);
        if (this._laterId)
            global.compositor.get_laters().remove(this._laterId);
        this._laterId = 0;
        // A bar's classes go in one change: each change restyles the whole bar
        for (const panel of this._ended ? [] : this._bars)
            this._glass.detach(panel, {also: [CLEAR]});
        for (const {panel} of this._ended ? [] : this._panels())
            panel.remove_style_class_name(CLEAR);
        this._bars = null;
        for (const button of this._ended ? [] : this._pills)
            this._glass.detach(button);
        this._pills = null;
    }

    // The main bar and the other screens' bars, each with its screen and the text its glass keeps readable
    _panels() {
        const others = this._glass._ext.modules?.get('bars')?.panels?.() ?? [];
        return [{panel: Main.panel, monitor: Main.layoutManager.primaryIndex, text}, ...others];
    }

    // The panes the style wants: one behind each bar, one behind each button, or none
    restyle() {
        if (!this._pills || this._ended)
            return;
        const style = this._glass.style;
        const bar = style?.look.bar ?? 'pane';
        const bare = bar === 'bare' && BARE_TEXT.has(style?.name);
        const panels = bar === 'pane' || bare ? this._panels() : [];
        const material = bare ? 'bare' : 'menu-bar';
        const panes = new Set(panels.map(({panel}) => panel));
        const screens = new Map(this._panels().map(({panel, monitor}) => [panel, monitor]));
        for (const panel of [...this._bars]) {
            if (!panes.has(panel)) {
                // Clear or not as _sync below leaves it, in the same change
                const clear = screens.has(panel) && this._clear(panel, screens.get(panel));
                this._glass.detach(panel, {also: clear ? [] : [CLEAR]});
                this._bars.delete(panel);
            }
        }
        for (const {panel, monitor, text: readable} of panels) {
            // A bare bar's veil, when it shows, is a capsule like the style's clock
            const look = {material, radius: bare ? 999 : null, inClones: true,
                text: bare ? (panel === Main.panel ? bareText : readable) : readable};
            // A bar already on glass takes the new look; attach() only gives its classes to a new one, clear or not
            // with them (one restyle of the bar rather than two)
            this._glass.attach(panel, look, {classes: ['amethystora-glass-bar'],
                also: this._clear(panel, monitor) ? [CLEAR] : []});
            this._bars.add(panel);
        }
        const wanted = new Set(bar === 'pills' && style ? style.buttons : []);
        for (const button of [...this._pills]) {
            if (!wanted.has(button)) {
                this._glass.detach(button);
                this._pills.delete(button);
            }
        }
        for (const button of wanted) {
            if (!this._pills.has(button)) {
                this._glass.attach(button, {material: 'menu-bar', radius: null, inClones: true, text: pillText},
                    {classes: ['amethystora-glass-pill']});
                this._pills.add(button);
            }
        }
        this._sync();
    }

    // Windows move every frame while they are dragged: the bar looks once per frame at most
    _queue() {
        if (this._laterId || this._ended)
            return;
        this._laterId = global.compositor.get_laters().add(Meta.LaterType.BEFORE_REDRAW, () => {
            this._laterId = 0;
            this._sync();
            return false;
        });
    }

    _sync() {
        if (!this._pills || this._ended)
            return;
        // The effects are made again when the glass stops being solid: they are asked for each time
        for (const actor of [...this._bars, ...this._pills]) {
            const effect = this._glass.effectOf(actor);
            if (effect)
                effect.enabled = !this._inOverview;
        }
        for (const {panel, monitor} of this._panels()) {
            if (this._clear(panel, monitor))
                panel.add_style_class_name(CLEAR);
            else
                panel.remove_style_class_name(CLEAR);
        }
    }

    // Each bar is clear until a window of its own screen comes near it
    _clear(panel, monitor) {
        return !this._glass.solid &&
            (this._inOverview || !Main.sessionMode.hasWindows || !this._touched(panel, monitor));
    }

    _touched(panel, monitor) {
        if (!panel.mapped)
            return false;
        const [, top] = panel.get_transformed_position();
        // As laid out: `height` is the height it asks for, margins included, from when a button is added until the next
        // frame, which took a window under the bar for one touching it, and nothing looks again
        const scale = St.ThemeContext.get_for_stage(global.stage).scale_factor;
        const limit = top + panel.allocation.get_height() + NEAR * scale;
        const active = global.workspace_manager.get_active_workspace();
        // With the tiling on several screens the bar's screen may show a workspace GNOME does not have, whose windows
        // are drawn as pictures (tiling/screens.js) while their own actors are hidden
        const engine = this._glass._ext.modules.get('tiling')?.engine;
        const workspace = engine?.screens?.shownOn(monitor) ?? active;
        for (const window of this._glass.windows?.windows ?? []) {
            if (!TYPES.has(window.get_window_type()) || window.is_override_redirect() || window.minimized ||
                !window.located_on_workspace(workspace) || window.get_monitor() !== monitor ||
                (window.get_wm_class() && leftAlone(window)))
                continue;
            // A picture leaves out what the tiling hides (a window scrolled off the strip), as the window's own actor
            // would be
            const seen = workspace === active ? window.get_compositor_private()?.visible : !engine?.info(window)?.hidden;
            if (!seen || !window.showing_on_its_workspace())
                continue;
            if (window.get_frame_rect().y < limit)
                return true;
        }
        return false;
    }
}
