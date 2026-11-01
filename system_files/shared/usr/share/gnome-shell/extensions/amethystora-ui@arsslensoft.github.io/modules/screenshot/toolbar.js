// The drawing toolbar at the top of the screenshot UI: the tools, the colours, the line width, undo and erase. It
// only shows what it is told and says what was clicked; the session decides what that means.

import Atk from 'gi://Atk';
import Clutter from 'gi://Clutter';
import GLib from 'gi://GLib';
import GObject from 'gi://GObject';
import St from 'gi://St';

import {gettext as _} from 'resource:///org/gnome/shell/extensions/extension.js';
import * as Slider from 'resource:///org/gnome/shell/ui/slider.js';

import {COLOURS, MAX_WIDTH, MIN_WIDTH, TOOLS} from './marks.js';

// Milliseconds the pointer rests on a button before its name shows, and the gap under the button
const TOOLTIP_DELAY = 500;
const TOOLTIP_GAP = 6;
// The swatches and the slider while no tool draws: there, but not offered
const DIMMED = 80;

// A name shown under a button of the toolbar, which sits at the top of the screen where GNOME's own tooltips,
// which open above, would be off it
const BelowTooltip = GObject.registerClass(
class BelowTooltip extends St.Label {
    _init(widget, text) {
        super._init({style_class: 'screenshot-ui-tooltip amethystora-screenshot-tooltip', text, visible: false});
        this._widget = widget;
        this._timeout = 0;
        widget.connectObject('notify::hover', () => {
            if (widget.hover)
                this._queue();
            else
                this._hide();
        }, 'destroy', () => this.destroy(), this);
        this.connect('destroy', () => this._cancel());
    }

    _cancel() {
        if (this._timeout)
            GLib.source_remove(this._timeout);
        this._timeout = 0;
    }

    _queue() {
        this._cancel();
        this._timeout = GLib.timeout_add(GLib.PRIORITY_DEFAULT, TOOLTIP_DELAY, () => {
            this._timeout = 0;
            const extents = this._widget.get_transformed_extents();
            this.show();
            const x = Math.round(extents.get_x() + (extents.get_width() - this.width) / 2);
            this.set_position(Math.clamp(x, 0, global.stage.width - this.width),
                Math.round(extents.get_y() + extents.get_height() + TOOLTIP_GAP));
            return GLib.SOURCE_REMOVE;
        });
    }

    _hide() {
        this._cancel();
        this.hide();
    }
});

const ToolButton = GObject.registerClass(
class ToolButton extends St.Button {
    _init(gicon, name) {
        super._init({
            style_class: 'amethystora-screenshot-tool',
            child: new St.Icon({gicon, icon_size: 16}),
            accessible_name: name,
            can_focus: true,
        });
    }

    setIcon(gicon) {
        this.child.gicon = gicon;
    }
});

export const Toolbar = GObject.registerClass({
    Signals: {
        'tool': {param_types: [GObject.TYPE_STRING]},
        'colour': {param_types: [GObject.TYPE_STRING]},
        'width': {param_types: [GObject.TYPE_DOUBLE]},
        'undo': {},
        'clear': {},
    },
}, class Toolbar extends St.BoxLayout {
    // `icon(name)` gives the picture of a tool; `tooltips` is the actor the names are shown in
    _init(icon, tooltips) {
        super._init({
            style_class: 'amethystora-screenshot-toolbar',
            reactive: true,
            // A bin places a child by its alignment only when the child asks for the room
            x_expand: true,
            y_expand: true,
            x_align: Clutter.ActorAlign.CENTER,
            y_align: Clutter.ActorAlign.START,
        });
        this._icon = icon;
        this._tooltips = tooltips;
        this._screen = false;

        const names = {
            move: _('Move or delete a mark'),
            pen: _('Pen'),
            box: _('Box'),
            fill: _('Filled box, to hide something'),
            highlight: _('Highlighter'),
            arrow: _('Arrow'),
            text: _('Text'),
            marker: _('Numbered marker'),
        };
        this._tools = new Map();
        for (const tool of TOOLS) {
            if (tool === 'pen')
                this._separator();
            const button = new ToolButton(icon(tool), names[tool] ?? '');
            button.connect('clicked', () => this.emit('tool', tool));
            this.add_child(button);
            this._tools.set(tool, button);
            if (tool !== 'area')
                this._tip(button, names[tool]);
        }
        this._areaTip = this._tip(this._tools.get('area'), '');
        this._separator();

        const colourNames = [_('White'), _('Black'), _('Red'), _('Orange'), _('Yellow'), _('Green'), _('Blue'),
            _('Purple')];
        this._controls = new St.BoxLayout({style_class: 'amethystora-screenshot-controls'});
        this._swatches = new Map();
        COLOURS.forEach((colour, i) => {
            const swatch = new St.Button({
                style_class: 'amethystora-screenshot-swatch',
                child: new St.Widget({
                    style_class: 'amethystora-screenshot-swatch-disc',
                    style: `background-color: ${colour};`,
                }),
                accessible_name: colourNames[i],
                accessible_role: Atk.Role.RADIO_BUTTON,
                can_focus: true,
            });
            swatch.connect('clicked', () => this.emit('colour', colour));
            this._controls.add_child(swatch);
            this._swatches.set(colour, swatch);
            this._tip(swatch, colourNames[i]);
        });
        this._controls.add_child(this._separator(true));
        this._slider = new Slider.Slider(0);
        this._slider.add_style_class_name('amethystora-screenshot-slider');
        this._slider.accessible_name = _('Line width');
        this._slider.connect('notify::value', () => {
            if (!this._setting)
                this.emit('width', MIN_WIDTH + this._slider.value * (MAX_WIDTH - MIN_WIDTH));
        });
        this._controls.add_child(this._slider);
        this.add_child(this._controls);
        this._separator();

        for (const [action, iconName, name] of [
            ['undo', 'edit-undo-symbolic', _('Undo')],
            ['clear', 'user-trash-symbolic', _('Erase everything')],
        ]) {
            const button = new ToolButton(null, name);
            button.child.icon_name = iconName;
            button.connect('clicked', () => this.emit(action));
            this.add_child(button);
            this._tip(button, name);
        }
        this.setScreenMode(false);
    }

    // A thin line between two groups; `detached` makes one for a group other than the toolbar itself
    _separator(detached = false) {
        const line = new St.Widget({style_class: 'amethystora-screenshot-separator', y_expand: true});
        if (!detached)
            this.add_child(line);
        return line;
    }

    _tip(widget, text) {
        const tip = new BelowTooltip(widget, text);
        this._tooltips.add_child(tip);
        return tip;
    }

    get slider() {
        return this._slider;
    }

    setTool(tool) {
        for (const [name, button] of this._tools)
            button.checked = name === tool;
    }

    setColour(colour) {
        this._colour = colour;
        for (const [name, swatch] of this._swatches) {
            const chosen = name === colour;
            swatch.checked = chosen && this._live;
            swatch.style = chosen && this._live ? `border-color: ${colour};` : null;
        }
    }

    setWidth(width) {
        this._setting = true;
        this._slider.value = (width - MIN_WIDTH) / (MAX_WIDTH - MIN_WIDTH);
        this._setting = false;
    }

    // The colours and the width are offered only while something would take them
    setLive(live) {
        this._live = live;
        this._controls.opacity = live ? 255 : DIMMED;
        this._controls.reactive = live;
        for (const swatch of this._swatches.values())
            swatch.reactive = live;
        this._slider.reactive = live;
        this.setColour(this._colour);
    }

    get live() {
        return Boolean(this._live);
    }

    // In screen mode the first tool picks a display, as GNOME's screen mode does on a click; in area mode it frames
    setScreenMode(screen) {
        this._screen = screen;
        const area = this._tools.get('area');
        area.setIcon(this._icon(screen ? 'display' : 'area'));
        const name = screen ? _('Pick a display') : _('Frame the area');
        area.accessible_name = name;
        this._areaTip.text = name;
    }
});
