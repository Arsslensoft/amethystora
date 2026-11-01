// A row of children that wraps onto the next line where the width runs out, as the picker's tabs do; right to left in
// a language written that way. Only the tabs use it: a dozen buttons, laid out when the picker opens.

import Clutter from 'gi://Clutter';
import GObject from 'gi://GObject';

export const WrapLayout = GObject.registerClass(
class AmethystoraWrapLayout extends Clutter.LayoutManager {
    _init(spacing = 0) {
        super._init();
        this._spacing = spacing;
    }

    // Each child's place for a width: [child, x, y, width, height], and the height they take
    _lines(container, width) {
        const places = [];
        let x = 0, y = 0, line = 0;
        for (const child of container) {
            if (!child.visible)
                continue;
            const [, childWidth] = child.get_preferred_width(-1);
            const [, childHeight] = child.get_preferred_height(childWidth);
            if (x > 0 && x + childWidth > width) {
                x = 0;
                y += line + this._spacing;
                line = 0;
            }
            places.push([child, x, y, childWidth, childHeight]);
            x += childWidth + this._spacing;
            line = Math.max(line, childHeight);
        }
        return [places, places.length ? y + line : 0];
    }

    vfunc_get_preferred_width(container) {
        let widest = 0, total = 0;
        for (const child of container) {
            if (!child.visible)
                continue;
            const [, width] = child.get_preferred_width(-1);
            widest = Math.max(widest, width);
            total += width + (total ? this._spacing : 0);
        }
        return [widest, total];
    }

    vfunc_get_preferred_height(container, forWidth) {
        const [, height] = this._lines(container, forWidth < 0 ? Infinity : forWidth);
        return [height, height];
    }

    vfunc_allocate(container, box) {
        const width = box.get_width();
        const rtl = container.get_text_direction() === Clutter.TextDirection.RTL;
        const [places] = this._lines(container, width);
        for (const [child, x, y, childWidth, childHeight] of places) {
            const left = rtl ? width - x - childWidth : x;
            child.allocate(new Clutter.ActorBox({x1: left, y1: y, x2: left + childWidth, y2: y + childHeight}));
        }
    }
});
