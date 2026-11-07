// The desktop style: the shape the whole shell is drawn in, the top bar, the menus and quick settings, the
// notifications, the dialogs and prompts, the on-screen alerts, the buttons and controls, the dock and the windows'
// panes. Its one key, `style` (org.gnome.shell.extensions.amethystora-ui.style), is written by amethystora-theme from
// the account's choice (ame desktop style) or the theme's style.theme. This module only says which style it is: it
// gives the shell's top group the class amethystora-style-<style>, which every rule of the theme's shell-styles.css
// (themed/shell-styles.css.tpl) starts from, and marks the top bar's buttons by place for the styles that colour or
// round them one by one. The glass asks it how the top bar is made and how round the windows are. The applications'
// half of each style is themed/style-<style>.gtk.css.tpl.

import Meta from 'gi://Meta';

import * as Main from 'resource:///org/gnome/shell/ui/main.js';

const SCHEMA = 'org.gnome.shell.extensions.amethystora-ui.style';
const PREFIX = 'amethystora-style-';
// The button classes the segments and the islands are drawn from: the first and last of each of the bar's three
// boxes, and a colour from one to six, counted along the bar
const FIRST = `${PREFIX}first`;
const LAST = `${PREFIX}last`;
const COLOURS = 6;

// What each style is made of, beyond its stylesheet. bar is how the glass treats the top bar: 'pane' is one pane
// behind the whole bar, with the bar's corners; 'pills' a pane behind each of its buttons; 'solid' and 'bare' none,
// the bar being either drawn solid or left out altogether so that only its buttons show. window is the corners of
// the windows' panes, which the style's GTK stylesheet gives the windows themselves.
export const STYLES = {
    islands: {bar: 'pills', window: 16},
    floating: {bar: 'pane', window: 16},
    hairline: {bar: 'solid', window: 6},
    capsule: {bar: 'bare', window: 18},
    terminal: {bar: 'solid', window: 0},
    segments: {bar: 'bare', window: 12},
    neon: {bar: 'pane', window: 10},
};
const DEFAULT = 'floating';

export class Style {
    constructor(ext) {
        this._ext = ext;
    }

    enable() {
        this._listeners = new Set();
        // The classes each button carries, and the containers watched for showing or hiding
        this._classes = new Map();
        this._shown = new Set();
        this._settings = this._ext.getSettings(SCHEMA);
        this._settings.connectObject('changed::style', () => this._sync(), this);
        this._watched = new Set();
        this._watchBoxes();
        this._sync();
    }

    disable() {
        this._settings?.disconnectObject(this);
        // Disposed now rather than by the collector (dock.js says why)
        this._settings?.run_dispose();
        this._settings = null;
        for (const box of this._watched ?? [])
            box.disconnectObject(this);
        this._watched = null;
        if (this._markLater)
            global.compositor.get_laters().remove(this._markLater);
        this._markLater = 0;
        this._unmark();
        // A screen lock stops the extension and the unlock starts it again: the class stays meanwhile (with the
        // stylesheets unloaded it matches nothing), since taking it off and putting it back each restyles the whole stage
        if (this.name && !Main.sessionMode.isLocked)
            Main.layoutManager.uiGroup.remove_style_class_name(PREFIX + this.name);
        this.name = null;
        this._listeners = null;
    }

    // What the current style is made of
    get look() {
        return STYLES[this.name] ?? STYLES[DEFAULT];
    }

    // The top bar's buttons, left to right, those showing
    get buttons() {
        return this._boxes().flatMap(box => box.get_children())
            .filter(container => container.visible)
            .map(container => container.get_first_child?.() ?? container)
            .filter(button => button && button.visible);
    }

    // Called when the style changes, and when the bar's buttons change while it is one that draws them one by one
    onChange(listener) {
        this._listeners.add(listener);
    }

    // The main bar's three boxes and those of the other screens' bars (modules/bars.js)
    _boxes() {
        const panel = Main.panel;
        const others = this._ext.modules?.get('bars')?.boxes?.() ?? [];
        return [panel._leftBox, panel._centerBox, panel._rightBox, ...others].filter(Boolean);
    }

    // The bars changed (another screen's bar came or went): their buttons are marked again
    refresh() {
        if (!this._settings)
            return;
        this._watchBoxes();
        this._mark();
    }

    _watchBoxes() {
        const boxes = new Set(this._boxes());
        for (const box of this._watched) {
            if (!boxes.has(box))
                box.disconnectObject(this);
        }
        for (const box of boxes) {
            if (this._watched.has(box))
                continue;
            box.connectObject(
                'child-added', () => this._queueMark(),
                'child-removed', () => this._queueMark(),
                'destroy', () => this._watched?.delete(box),
                this);
        }
        this._watched = boxes;
    }

    _sync() {
        let name = this._settings.get_string('style');
        if (!STYLES[name])
            name = DEFAULT;
        const group = Main.layoutManager.uiGroup;
        if (this.name !== name) {
            // In one change of the class string (one restyle of the whole stage, not one to take the old style off and
            // another to put the new one on), and whatever style a lock left there (disable) replaced with it
            const styles = Object.keys(STYLES).map(each => PREFIX + each);
            const classes = (group.get_style_class_name() ?? '').split(/\s+/)
                .filter(each => each && !styles.includes(each));
            const wanted = [...classes, PREFIX + name].join(' ');
            if (wanted !== group.get_style_class_name())
                group.set_style_class_name(wanted);
            this.name = name;
        }
        this._mark();
    }

    _unmark() {
        for (const [button, names] of this._classes) {
            button.disconnectObject(this);
            for (const name of names)
                button.remove_style_class_name(name);
        }
        this._classes.clear();
        for (const container of this._shown)
            container.disconnectObject(this);
        this._shown.clear();
    }

    // Each button by its place: the first and last of its box, and its colour along the bar. A button that hides or
    // shows moves the others, so each one showing or hiding marks them all again. Only the classes that change are
    // touched: each change restyles the button's whole tree.
    _mark() {
        const wanted = new Map();
        let colour = 0;
        for (const box of this._boxes()) {
            const buttons = box.get_children().filter(c => c.visible)
                .map(c => c.get_first_child?.() ?? c).filter(Boolean);
            buttons.forEach((button, i) => {
                const names = new Set([`${PREFIX}colour${colour++ % COLOURS + 1}`]);
                if (i === 0)
                    names.add(FIRST);
                if (i === buttons.length - 1)
                    names.add(LAST);
                wanted.set(button, names);
            });
            for (const container of box.get_children()) {
                if (this._shown.has(container))
                    continue;
                container.connectObject(
                    'notify::visible', () => this._queueMark(),
                    'destroy', () => this._shown?.delete(container),
                    this);
                this._shown.add(container);
            }
        }
        for (const [button, names] of this._classes) {
            const keep = wanted.get(button);
            for (const name of names) {
                if (!keep?.has(name))
                    button.remove_style_class_name(name);
            }
            if (!keep) {
                button.disconnectObject(this);
                this._classes.delete(button);
            }
        }
        for (const [button, names] of wanted) {
            const had = this._classes.get(button);
            for (const name of names) {
                if (!had?.has(name))
                    button.add_style_class_name(name);
            }
            if (!had)
                button.connectObject('destroy', () => this._classes?.delete(button), this);
            this._classes.set(button, names);
        }
        for (const listener of this._listeners)
            listener();
    }

    // Many buttons show or hide at once (the tray, the session's start): marked once, after them
    _queueMark() {
        if (this._markLater)
            return;
        this._markLater = global.compositor.get_laters().add(Meta.LaterType.BEFORE_REDRAW, () => {
            this._markLater = 0;
            if (this._settings)
                this._mark();
            return false;
        });
    }
}
