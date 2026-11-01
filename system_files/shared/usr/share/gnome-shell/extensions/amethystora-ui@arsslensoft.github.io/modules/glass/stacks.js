// Stacked notifications in the drawer. GNOME stacks a source's notifications with the newest on top and a strip of
// each card below it showing; on glass the cards are see-through, so the cards underneath would show through the top
// one. Each card under another is cut to the strip that shows below the card above it and its contents are hidden,
// and as a group opens the strips grow with it while each card's contents fade in, every later card a little after
// the one before. The group's header, which lies behind the stack, fades in with the group.

const GROUP = 'message-notification-group';
// How much later each card further down the stack starts to show its contents, as a share of the opening
const LATER = 0.15;

export class Stacks {
    constructor() {
        this._views = new Set();
        this._groups = new Set();
    }

    // A notification list, and every group it has or is given; the list holds each group in a bin of its own
    watch(view) {
        if (this._views.has(view))
            return;
        this._views.add(view);
        const keep = child => this._keep(child.child ?? child);
        view.connectObject(
            'child-added', (actor, child) => keep(child),
            'destroy', () => this._views.delete(view),
            this);
        view.get_children().forEach(keep);
    }

    destroy() {
        for (const view of this._views)
            view.disconnectObject(this);
        this._views.clear();
        for (const group of this._groups)
            this._release(group);
        this._groups.clear();
    }

    _keep(group) {
        if (this._groups.has(group) || !group.has_style_class_name?.(GROUP))
            return;
        this._groups.add(group);
        const sync = () => this._sync(group);
        group.layout_manager.connectObject('notify::expansion', sync, this);
        group.connectObject(
            'child-added', (actor, child) => {
                child.connectObject('notify::allocation', sync, this);
                sync();
            },
            'child-removed', (actor, child) => {
                child.disconnectObject(this);
                this._show(child);
                sync();
            },
            'destroy', () => {
                this._groups.delete(group);
                group.layout_manager?.disconnectObject(this);
            },
            this);
        group.get_children().forEach(child => child.connectObject('notify::allocation', sync, this));
        sync();
    }

    _release(group) {
        group.layout_manager?.disconnectObject(this);
        group.disconnectObject(this);
        for (const child of group.get_children()) {
            child.disconnectObject(this);
            this._show(child);
        }
    }

    _show(child) {
        child.remove_clip();
        child.opacity = 255;
        const content = child.child?.child;
        if (content)
            content.opacity = 255;
    }

    // GNOME keeps the newest card first, its cover second and its header last, and paints them in reverse
    _sync(group) {
        const children = group.get_children();
        if (children.length < 3)
            return;
        const header = children.at(-1);
        const [top, , ...stack] = children.slice(0, -1);
        const expansion = group.layout_manager.expansion ?? (group.expanded ? 1 : 0);
        header.opacity = Math.round(255 * expansion);

        let above = top;
        stack.forEach((item, i) => {
            const box = item.get_allocation_box();
            const height = box.get_height();
            const hidden = above.get_allocation_box().y2 - box.y1;
            if (hidden <= 0)
                item.remove_clip();
            else if (hidden >= height)
                item.set_clip(0, 0, 0, 0);
            else
                item.set_clip(0, hidden, box.get_width(), height - hidden);

            const content = item.child?.child;
            if (content) {
                const start = LATER * i;
                content.opacity = Math.round(255 * Math.min(Math.max((expansion - start) / (1 - start), 0), 1));
            }
            above = item;
        });
    }
}
