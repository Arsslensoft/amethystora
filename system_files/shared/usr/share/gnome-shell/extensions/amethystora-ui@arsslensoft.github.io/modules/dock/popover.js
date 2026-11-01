// A card that holds the keyboard while it is open (a stack, a folder): it closes on a click outside it, on Escape,
// or on a click on its item again, and the arrow keys move between what is in it.

import Clutter from 'gi://Clutter';
import Shell from 'gi://Shell';
import St from 'gi://St';

import * as Main from 'resource:///org/gnome/shell/ui/main.js';

import {placeCard, popIn, popOut} from './cards.js';
import {inside, stageRect} from './util.js';

const ARROWS = {
    [Clutter.KEY_Left]: St.DirectionType.LEFT,
    [Clutter.KEY_Right]: St.DirectionType.RIGHT,
    [Clutter.KEY_Up]: St.DirectionType.UP,
    [Clutter.KEY_Down]: St.DirectionType.DOWN,
};

export class Popover {
    // pop: {from, scale, open, close, openMode, closeMode}
    constructor(dock, dash, state, {styleClass, gap, pop, onClose}) {
        this._dock = dock;
        this._dash = dash;
        this.state = state;
        this._gap = gap;
        this._pop = pop;
        this._onClose = onClose;
        this.card = new St.BoxLayout({
            style_class: styleClass,
            orientation: Clutter.Orientation.VERTICAL,
            reactive: true,
        });
    }

    open() {
        const card = this.card;
        Main.layoutManager.addChrome(card);
        this.place();
        popIn(card, this._dash, {
            from: this._pop.from, scale: this._pop.scale, duration: this._pop.open, mode: this._pop.openMode,
            level: this._dock.motionLevel(),
        });
        this._grab = Main.pushModal(card, {actionMode: Shell.ActionMode.POPUP});
        card.connect('button-press-event', (actor, event) => this._pressed(event));
        card.connect('touch-event', (actor, event) => {
            if (event.type() === Clutter.EventType.TOUCH_BEGIN)
                return this._pressed(event);
            return Clutter.EVENT_PROPAGATE;
        });
        card.connect('key-press-event', (actor, event) => this._key(event));
        this._dock.glassChanged();
    }

    // Placed again once its contents are known
    place() {
        placeCard(this.card, this._dash, this.state.restCentre(), this._gap);
    }

    focusFirst(actor) {
        actor?.grab_key_focus();
    }

    _pressed(event) {
        const [x, y] = event.get_coords();
        if (!inside(stageRect(this.card), x, y)) {
            this.close();
            return Clutter.EVENT_STOP;
        }
        return Clutter.EVENT_PROPAGATE;
    }

    _key(event) {
        const key = event.get_key_symbol();
        if (key === Clutter.KEY_Escape) {
            this.close();
            return Clutter.EVENT_STOP;
        }
        const direction = ARROWS[key];
        if (direction !== undefined) {
            const focused = global.stage.key_focus;
            if (focused && this.card.contains(focused))
                this.card.navigate_focus(focused, direction, false);
            else
                this.card.navigate_focus(null, St.DirectionType.TAB_FORWARD, false);
            return Clutter.EVENT_STOP;
        }
        return Clutter.EVENT_PROPAGATE;
    }

    get isOpen() {
        return Boolean(this._grab);
    }

    close(now = false) {
        if (this._closed)
            return;
        this._closed = true;
        if (this._grab) {
            Main.popModal(this._grab);
            this._grab = null;
        }
        const card = this.card;
        if (now) {
            card.destroy();
        } else {
            popOut(card, this._dash, {
                to: this._pop.from, scale: this._pop.scale, duration: this._pop.close, mode: this._pop.closeMode,
                level: this._dock.motionLevel(), onComplete: () => this._dock.glassChanged(),
            });
        }
        this._onClose?.();
        this._dock.glassChanged();
    }
}
