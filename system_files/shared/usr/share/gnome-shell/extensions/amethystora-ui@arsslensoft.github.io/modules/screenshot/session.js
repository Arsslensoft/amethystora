// One opening of GNOME's screenshot UI, from the moment it opens to the moment it closes: the toolbar, the marks and
// the layers they are drawn on and taken from, the text field, the size of the area being framed, and the Extract Text
// button. Nothing outlives the opening: closing the UI drops every mark.

import Clutter from 'gi://Clutter';
import GLib from 'gi://GLib';
import Meta from 'gi://Meta';
import St from 'gi://St';

import {gettext as _} from 'resource:///org/gnome/shell/extensions/extension.js';
import * as Layout from 'resource:///org/gnome/shell/ui/layout.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import * as ScreenshotUI from 'resource:///org/gnome/shell/ui/screenshot.js';

import {
    DRAWING, MAX_WIDTH, MIN_WIDTH, TOOLS, isEmpty, markAt, measureText, moveMark, newMark, paintMarks, paintPicked,
    pickBox, textSize,
} from './marks.js';
import {Toolbar} from './toolbar.js';

// The keys that choose the tools, in the toolbar's order: the number row, the same row of a French or Belgian
// keyboard without Shift, and a letter each
const NUMBER_KEYS = [Clutter.KEY_1, Clutter.KEY_2, Clutter.KEY_3, Clutter.KEY_4, Clutter.KEY_5, Clutter.KEY_6,
    Clutter.KEY_7, Clutter.KEY_8, Clutter.KEY_9];
const AZERTY_KEYS = [Clutter.KEY_ampersand, Clutter.KEY_eacute, Clutter.KEY_quotedbl, Clutter.KEY_apostrophe,
    Clutter.KEY_parenleft, Clutter.KEY_section, Clutter.KEY_egrave, Clutter.KEY_exclam, Clutter.KEY_ccedilla];
const LETTERS = 'qdfrbhatn';
// What the toolbar fades to while an area is being framed under it, and the size readout's fade
const FRAMING_OPACITY = 100;
const READOUT_FADE = 400;
// The size readout says nothing about an area smaller than this, in logical pixels
const READOUT_MIN = [100, 50];
const MODIFIERS = Clutter.ModifierType.CONTROL_MASK | Clutter.ModifierType.MOD1_MASK |
    Clutter.ModifierType.SUPER_MASK | Clutter.ModifierType.HYPER_MASK | Clutter.ModifierType.META_MASK;

export class Session {
    // `module` gives icons, the tools' remembered colours and widths, and takes the captures; `portal` says the UI
    // was opened for an app through the screenshot portal
    constructor(module, ui, portal) {
        this._module = module;
        this._ui = ui;
        this._portal = portal;
        this._marks = [];
        this._tool = 'area';
        this._picked = null;
        this._stroke = null;
        this._entry = null;
        this._colour = '#000000';
        this._width = 4;
        this._handles = ui._areaSelector.get_children()
            .filter(child => child.has_style_class_name('screenshot-ui-area-selector-handle'));

        this._build();
        this._chooseTool('area');
        this._sync();
    }

    _build() {
        const ui = this._ui;
        const fill = {x_expand: true, y_expand: true};

        // The marks, under GNOME's area selector so that its shade covers what will not be in the picture
        this._paint = new St.Widget(fill);
        this._areas = Main.layoutManager.monitors.map(monitor => {
            const area = new St.DrawingArea({
                x: monitor.x,
                y: monitor.y,
                width: monitor.width,
                height: monitor.height,
            });
            area.connect('repaint', () => this._repaint(area, monitor));
            this._paint.add_child(area);
            return area;
        });
        ui.insert_child_below(this._paint, ui._areaSelector);

        // What the pointer draws on, over the area selector and the screen selectors, under GNOME's panel
        this._input = new St.Widget({...fill, reactive: false});
        this._input.connectObject(
            'button-press-event', (a, event) => this._onPress(event),
            'motion-event', (a, event) => this._onMotion(event),
            'button-release-event', (a, event) => this._onRelease(event),
            'touch-event', (a, event) => this._onTouch(event), this);
        ui.insert_child_below(this._input, ui._primaryMonitorBin);

        // The text field, the delete button, the size readout and the toolbar's names, over both
        this._overlay = new St.Widget(fill);
        ui.insert_child_above(this._overlay, this._input);

        this._delete = new St.Button({
            style_class: 'amethystora-screenshot-round amethystora-screenshot-delete',
            child: new St.Icon({icon_name: 'user-trash-symbolic', icon_size: 16}),
            accessible_name: _('Delete the mark'),
            visible: false,
        });
        this._delete.connect('clicked', () => this._deletePicked());
        this._overlay.add_child(this._delete);

        this._readout = new St.Label({style_class: 'amethystora-screenshot-readout', opacity: 0, visible: false});
        this._overlay.add_child(this._readout);

        this._toolbarBin = new St.Widget({layout_manager: new Clutter.BinLayout()});
        this._toolbarBin.add_constraint(new Layout.MonitorConstraint({primary: true}));
        this._toolbar = new Toolbar(name => this._module.icon(name), this._overlay);
        this._toolbar.connectObject(
            'tool', (t, tool) => this._chooseTool(tool),
            'colour', (t, colour) => this._setColour(colour),
            'width', (t, width) => this._setWidth(width),
            'undo', () => this.undo(),
            'clear', () => this._clear(), this);
        this._toolbarBin.add_child(this._toolbar);
        ui.add_child(this._toolbarBin);
        // The names over the toolbar
        ui.set_child_above_sibling(this._overlay, null);

        // Extract Text, beside GNOME's show-pointer toggle, when Gradia is there to read the text
        this._compact = ui._showPointerButton;
        this._compact.add_style_class_name('amethystora-screenshot-compact');
        if (this._module.gradiaInstalled()) {
            this._extract = new St.Button({
                style_class: 'screenshot-ui-show-pointer-button amethystora-screenshot-compact',
                child: new St.Icon({gicon: this._module.icon('extract'), icon_size: 16}),
                accessible_name: _('Extract Text'),
            });
            this._extract.connect('clicked', () => this._module.extractText());
            ui._showPointerButtonContainer.insert_child_at_index(this._extract, 0);
            this._extractTip = new ScreenshotUI.Tooltip(this._extract, {
                text: _('Extract Text'),
                style_class: 'screenshot-ui-tooltip',
                visible: false,
            });
            ui.add_child(this._extractTip);
        }
        this.syncPin(false);

        for (const button of [ui._selectionButton, ui._screenButton, ui._windowButton, ui._castButton])
            button.connectObject('notify::checked', () => this._sync(), this);
        ui._areaSelector.connectObject(
            'drag-started', () => this._onFraming(true),
            'drag-ended', () => this._onFraming(false),
            'motion-event', () => this._queueReadout(), this);
    }

    destroy() {
        this._closeEntry(false, false);
        this._endStroke();
        if (this._readoutLater)
            global.compositor.get_laters().remove(this._readoutLater);
        this._readoutLater = 0;
        const ui = this._ui;
        for (const button of [ui._selectionButton, ui._screenButton, ui._windowButton, ui._castButton])
            button.disconnectObject(this);
        ui._areaSelector.disconnectObject(this);
        for (const handle of this._handles)
            handle.show();
        this._compact.remove_style_class_name('amethystora-screenshot-compact');
        this._extractTip?.destroy();
        this._extract?.destroy();
        this._pinTip?.destroy();
        this._pin?.destroy();
        this._pinTip = this._pin = null;
        this._toolbarBin.destroy();
        this._overlay.destroy();
        this._input.destroy();
        this._paint.destroy();
        this._marks = [];
        this._picked = null;
    }

    // In area and screen mode of a screenshot the toolbar is out and the marks are drawn; never in window mode or
    // while recording
    get drawing() {
        const ui = this._ui;
        return ui._shotButton.checked && (ui._selectionButton.checked || ui._screenButton.checked);
    }

    // Extract Text reads an area only, and never for an app that asked for a screenshot
    get extractLive() {
        return Boolean(this._extract) && !this._portal && this._ui._shotButton.checked &&
            this._ui._selectionButton.checked;
    }

    // Pin takes an area or the screen, never a window, a recording, or what an app asked for
    get pinLive() {
        return Boolean(this._pin) && !this._portal && this._ui._shotButton.checked &&
            (this._ui._selectionButton.checked || this._ui._screenButton.checked);
    }

    // Pin, beside Extract Text, while pinning is on (capture-pin); made or taken away as the key changes
    syncPin(resync = true) {
        const ui = this._ui;
        const wanted = this._module.pinOn;
        if (wanted && !this._pin) {
            this._pin = new St.Button({
                style_class: 'screenshot-ui-show-pointer-button amethystora-screenshot-compact',
                child: new St.Icon({gicon: this._module.icon('pin'), icon_size: 16}),
                accessible_name: _('Pin on top'),
            });
            this._pin.connect('clicked', () => this._module.pin());
            const at = this._extract ? ui._showPointerButtonContainer.get_children().indexOf(this._extract) + 1 : 0;
            ui._showPointerButtonContainer.insert_child_at_index(this._pin, at);
            this._pinTip = new ScreenshotUI.Tooltip(this._pin, {
                text: _('Pin on top'),
                style_class: 'screenshot-ui-tooltip',
                visible: false,
            });
            ui.add_child(this._pinTip);
        } else if (!wanted && this._pin) {
            this._pinTip?.destroy();
            this._pin.destroy();
            this._pinTip = this._pin = null;
        }
        if (resync)
            this._sync();
    }

    get marks() {
        return this.drawing ? this._marks : [];
    }

    get portal() {
        return this._portal;
    }

    setPortal(portal) {
        this._portal = portal;
        this._sync();
    }

    _sync() {
        const ui = this._ui;
        const drawing = this.drawing;
        if (!drawing) {
            this._closeEntry(true);
            this._drop();
        }
        this._toolbarBin.visible = drawing;
        this._paint.visible = drawing;
        this._input.reactive = drawing && this._tool !== 'area';
        // With a tool that draws, dragging draws instead of framing, and the selector's handles are put away
        const drawsOverArea = drawing && ui._selectionButton.checked && this._tool !== 'area';
        for (const handle of this._handles)
            handle.visible = !drawsOverArea;
        this._toolbar.setScreenMode(ui._screenButton.checked);
        this._toolbar.setLive(DRAWING.has(this._tool) || (this._tool === 'move' && this._picked !== null));
        const cursors = {
            area: Clutter.CursorType.INHERIT,
            move: Clutter.CursorType.MOVE,
            text: Clutter.CursorType.TEXT,
        };
        this._input.cursor_type = cursors[this._tool] ?? Clutter.CursorType.CROSSHAIR;

        if (this._extract) {
            const live = this.extractLive;
            this._extract.reactive = live;
            this._extract.ease({opacity: live ? 255 : 80, duration: 200, mode: Clutter.AnimationMode.EASE_OUT_QUAD});
        }
        if (this._pin) {
            const live = this.pinLive;
            this._pin.reactive = live;
            this._pin.ease({opacity: live ? 255 : 80, duration: 200, mode: Clutter.AnimationMode.EASE_OUT_QUAD});
        }
    }

    // While an area is dragged out, the toolbar fades so that it hides nothing, and the area's size is shown
    _onFraming(started) {
        this._toolbar.ease({
            opacity: started ? FRAMING_OPACITY : 255,
            duration: 200,
            mode: Clutter.AnimationMode.EASE_OUT_QUAD,
        });
        this._framing = started;
        if (started)
            this._readout.show();
        this._updateReadout();
        this._readout.ease({
            opacity: started ? 255 : 0,
            duration: READOUT_FADE,
            mode: Clutter.AnimationMode.EASE_OUT_QUAD,
            onStopped: () => {
                if (!this._framing)
                    this._readout.hide();
            },
        });
    }

    // The selector moves the area as it handles the pointer: read it once that is done, before the frame is drawn
    _queueReadout() {
        if (!this._framing || this._readoutLater)
            return;
        this._readoutLater = global.compositor.get_laters().add(Meta.LaterType.BEFORE_REDRAW, () => {
            this._readoutLater = 0;
            this._updateReadout();
            return false;
        });
    }

    _updateReadout() {
        const [x, y, w, h] = this._ui._areaSelector.getGeometry();
        // In the pixels of the sharpest monitor, as the picture will have them
        const scale = Math.max(1, ...Main.layoutManager.monitors.map(m => global.display.get_monitor_scale(m.index)));
        this._readout.visible = this._framing && w >= READOUT_MIN[0] && h >= READOUT_MIN[1];
        this._readout.text = `${Math.round(w * scale)} × ${Math.round(h * scale)}`;
        this._readout.set_position(Math.round(x + (w - this._readout.width) / 2),
            Math.round(y + (h - this._readout.height) / 2));
    }

    _repaint(area, monitor) {
        const cr = area.get_context();
        paintMarks(cr, this._marks, monitor.x, monitor.y, 1);
        if (this._picked)
            paintPicked(cr, this._picked, monitor.x, monitor.y);
        cr.$dispose();
    }

    _redraw() {
        for (const area of this._areas)
            area.queue_repaint();
        this._placeDelete();
    }

    // The tools' colours and widths: each drawing tool keeps its own pair, saved as it is changed and taken back
    // when the tool is chosen again; a tool with none keeps what was current
    _chooseTool(tool) {
        this._closeEntry(true);
        this._drop();
        if (DRAWING.has(this._tool))
            this._module.rememberPair(this._tool, this._colour, this._width);
        this._tool = tool;
        const pair = this._module.pairOf(tool);
        if (pair)
            [this._colour, this._width] = pair;
        this._toolbar.setTool(tool);
        this._toolbar.setColour(this._colour);
        this._toolbar.setWidth(this._width);
        this._sync();
    }

    _setColour(colour) {
        this._colour = colour;
        this._toolbar.setColour(colour);
        this._changed();
    }

    _setWidth(width) {
        this._width = Math.clamp(width, MIN_WIDTH, MAX_WIDTH);
        this._changed();
    }

    // A change of colour or width goes to the tool, the picked-up mark and the open text field
    _changed() {
        if (DRAWING.has(this._tool))
            this._module.rememberPair(this._tool, this._colour, this._width);
        if (this._picked) {
            this._picked.colour = this._colour;
            this._picked.width = this._width;
            if (this._picked.tool === 'text')
                this._picked.extent = measureText(this._picked.text, textSize(this._width));
            this._redraw();
        }
        if (this._entry)
            this._styleEntry();
    }

    // One step of the wheel, a pixel more or less
    scrollWidth(step) {
        if (!this.drawing || !this._toolbar.live)
            return;
        const width = Math.clamp(Math.round(this._width) + step, MIN_WIDTH, MAX_WIDTH);
        this._toolbar.setWidth(width);
        this._setWidth(width);
    }

    get slider() {
        return this._toolbar.slider;
    }

    // Takes back the text field, then the picked-up mark, then the last mark drawn
    undo() {
        if (this._entry) {
            this._closeEntry(false);
            return;
        }
        if (this._tool === 'move' && this._picked) {
            this._deletePicked();
            return;
        }
        if (this._marks.pop()) {
            this._picked = null;
            this._redraw();
        }
    }

    _clear() {
        this._closeEntry(false);
        this._marks = [];
        this._drop();
        this._redraw();
    }

    _deletePicked() {
        if (!this._picked)
            return;
        this._marks = this._marks.filter(mark => mark !== this._picked);
        this._drop();
    }

    _drop() {
        if (!this._picked && !this._delete.visible)
            return;
        this._picked = null;
        this._dragFrom = null;
        this._sync();
        this._redraw();
    }

    // The delete button sits on the picked-up mark's top-right corner
    _placeDelete() {
        this._delete.visible = this._picked !== null;
        if (!this._picked)
            return;
        const [, y1, x2] = pickBox(this._picked);
        this._delete.set_position(Math.round(x2 - this._delete.width / 2), Math.round(y1 - this._delete.height / 2));
    }

    // Everything drawn, the open text field included, as the picture is about to be taken
    commit() {
        this._closeEntry(true);
        this._endStroke();
    }

    handleKey(event) {
        const symbol = event.get_key_symbol();
        const state = event.get_state();
        const ctrl = (state & Clutter.ModifierType.CONTROL_MASK) !== 0;

        // The text field has the keyboard: it types, and only Escape and Ctrl+Z are ours
        if (this._entry) {
            if (symbol === Clutter.KEY_Escape || (ctrl && (symbol === Clutter.KEY_z || symbol === Clutter.KEY_Z))) {
                this._closeEntry(false);
                return Clutter.EVENT_STOP;
            }
            return Clutter.EVENT_PROPAGATE;
        }
        if (ctrl && (symbol === Clutter.KEY_z || symbol === Clutter.KEY_Z)) {
            this.undo();
            return Clutter.EVENT_STOP;
        }
        if (!this.drawing || (state & MODIFIERS) !== 0)
            return Clutter.EVENT_PROPAGATE;
        if ((symbol === Clutter.KEY_Delete || symbol === Clutter.KEY_BackSpace) && this._tool === 'move' &&
            this._picked) {
            this._deletePicked();
            return Clutter.EVENT_STOP;
        }
        let index = NUMBER_KEYS.indexOf(symbol);
        if (index < 0)
            index = AZERTY_KEYS.indexOf(symbol);
        if (index < 0 && symbol < 0x80)
            index = LETTERS.indexOf(String.fromCharCode(symbol).toLowerCase());
        if (index < 0)
            return Clutter.EVENT_PROPAGATE;
        this._chooseTool(TOOLS[index]);
        return Clutter.EVENT_STOP;
    }

    // The pointer: the primary button or a touch draws, the other buttons do nothing
    _onPress(event) {
        // A finger is drawing: a second grab on top of its would never be let go of
        if (event.get_button() !== Clutter.BUTTON_PRIMARY || this._grab)
            return Clutter.EVENT_STOP;
        return this._begin(event.get_coords(), null);
    }

    _onMotion(event) {
        if (this._sequence)
            return Clutter.EVENT_PROPAGATE;
        return this._extend(event.get_coords());
    }

    _onRelease(event) {
        if (event.get_button() !== Clutter.BUTTON_PRIMARY || this._sequence)
            return Clutter.EVENT_STOP;
        this._endStroke();
        return Clutter.EVENT_STOP;
    }

    _onTouch(event) {
        const sequence = event.get_event_sequence();
        switch (event.type()) {
        case Clutter.EventType.TOUCH_BEGIN:
            if (this._grab)
                return Clutter.EVENT_STOP;
            return this._begin(event.get_coords(), sequence);
        case Clutter.EventType.TOUCH_UPDATE:
            if (this._sequence?.get_slot() === sequence?.get_slot())
                this._extend(event.get_coords());
            return Clutter.EVENT_STOP;
        case Clutter.EventType.TOUCH_END:
        case Clutter.EventType.TOUCH_CANCEL:
            if (this._sequence?.get_slot() === sequence?.get_slot())
                this._endStroke();
            return Clutter.EVENT_STOP;
        }
        return Clutter.EVENT_PROPAGATE;
    }

    _begin([x, y], sequence) {
        // A click elsewhere while the text field is open places the text, and opens nothing new
        if (this._entry) {
            this._closeEntry(true);
            return Clutter.EVENT_STOP;
        }
        switch (this._tool) {
        case 'move': {
            const mark = markAt(this._marks, x, y);
            if (!mark) {
                this._drop();
                return Clutter.EVENT_STOP;
            }
            this._picked = mark;
            this._colour = mark.colour;
            this._width = mark.width;
            this._toolbar.setColour(mark.colour);
            this._toolbar.setWidth(mark.width);
            this._dragFrom = [x, y];
            this._sync();
            this._redraw();
            break;
        }
        case 'text':
            this._openEntry(x, y);
            return Clutter.EVENT_STOP;
        case 'marker':
            this._marks.push(newMark('marker', this._colour, this._width, [x, y]));
            this._redraw();
            return Clutter.EVENT_STOP;
        default:
            this._stroke = newMark(this._tool, this._colour, this._width, [x, y]);
            this._marks.push(this._stroke);
        }
        this._sequence = sequence;
        this._grab = global.stage.grab(this._input);
        return Clutter.EVENT_STOP;
    }

    _extend([x, y]) {
        if (this._stroke) {
            if (this._stroke.tool === 'pen' || this._stroke.tool === 'highlight')
                this._stroke.points.push([x, y]);
            else
                this._stroke.points = [this._stroke.points[0], [x, y]];
            this._redraw();
            return Clutter.EVENT_STOP;
        }
        if (this._picked && this._dragFrom) {
            moveMark(this._picked, x - this._dragFrom[0], y - this._dragFrom[1]);
            this._dragFrom = [x, y];
            this._redraw();
            return Clutter.EVENT_STOP;
        }
        return Clutter.EVENT_PROPAGATE;
    }

    _endStroke() {
        this._grab?.dismiss();
        this._grab = null;
        this._sequence = null;
        this._dragFrom = null;
        if (this._stroke && isEmpty(this._stroke))
            this._marks = this._marks.filter(mark => mark !== this._stroke);
        this._stroke = null;
        this._redraw();
    }

    // A one-line field at the click, in the chosen colour; the text becomes a mark when it is placed. The field's
    // frame goes round the text, so that what is typed starts at the click and does not move when it is placed.
    _openEntry(x, y) {
        this._entry = new St.Entry({style_class: 'amethystora-screenshot-text', can_focus: true});
        this._entryAt = [x, y];
        this._styleEntry();
        this._overlay.add_child(this._entry);
        const node = this._entry.get_theme_node();
        this._entry.set_position(x - node.get_padding(St.Side.LEFT) - node.get_border_width(St.Side.LEFT),
            y - node.get_padding(St.Side.TOP) - node.get_border_width(St.Side.TOP));
        const text = this._entry.clutter_text;
        // Placed once the event that ends the field is over: focus also goes when the UI closes and unmaps the field,
        // and Clutter must not see it destroyed in the middle of that
        text.connectObject(
            'activate', () => this._queuePlace(),
            'key-focus-out', () => this._queuePlace(), this);
        text.grab_key_focus();
    }

    _queuePlace() {
        if (this._placeLater)
            return;
        this._placeLater = GLib.idle_add(GLib.PRIORITY_DEFAULT, () => {
            this._placeLater = 0;
            this._closeEntry(true);
            return GLib.SOURCE_REMOVE;
        });
    }

    _styleEntry() {
        const size = textSize(this._width);
        this._entry.style = `font-size: ${size}px; color: ${this._colour}; caret-color: ${this._colour};` +
            `min-width: ${4 * size}px;`;
    }

    // `refocus` gives the keyboard back to the screenshot UI, which is not wanted once it has closed
    _closeEntry(place, refocus = true) {
        if (this._placeLater)
            GLib.source_remove(this._placeLater);
        this._placeLater = 0;
        const entry = this._entry;
        if (!entry)
            return;
        this._entry = null;
        entry.clutter_text.disconnectObject(this);
        const text = entry.get_text().trim();
        entry.destroy();
        if (place && text) {
            const mark = newMark('text', this._colour, this._width, this._entryAt);
            mark.text = text;
            mark.extent = measureText(text, textSize(this._width));
            this._marks.push(mark);
            this._redraw();
        }
        // The screenshot UI's keys work again
        if (refocus && this._ui.visible)
            this._ui.grab_key_focus();
    }
}
