// The count on a stack of notifications: GNOME already folds an app's notifications in the calendar's list into one
// stack; on a folded stack of two or more, the top card's app name is followed by "· 3". It is put in while the calendar is
// open and taken out when it closes, so nothing is kept while it is shut.

import St from 'gi://St';

import * as Main from 'resource:///org/gnome/shell/ui/main.js';

const LABEL = 'amethystora-pill-stack-count';

export class Stacks {
    enable() {
        this._groups = new Map();
        this._menu = Main.panel.statusArea.dateMenu?.menu ?? null;
        this._menu?.connectObject('open-state-changed', (menu, open) => (open ? this._build() : this._drop()), this);
        if (this._menu?.isOpen)
            this._build();
    }

    disable() {
        this._menu?.disconnectObject(this);
        this._menu = null;
        this._drop();
        this._groups = null;
    }

    _view() {
        return Main.panel.statusArea.dateMenu?._messageList?._messageView ?? null;
    }

    _build() {
        const view = this._view();
        if (!view)
            return;
        view.connectObject('child-added', () => this._scan(), 'child-removed', () => this._scan(), this);
        this._scan();
    }

    _drop() {
        this._view()?.disconnectObject(this);
        for (const group of this._groups?.keys() ?? [])
            this._forget(group);
        this._groups?.clear();
    }

    // The view's notification groups, wherever they sit under it
    _scan() {
        const found = new Set();
        const visit = (actor, depth) => {
            for (const child of actor.get_children()) {
                if (child.has_style_class_name?.('message-notification-group'))
                    found.add(child);
                else if (depth < 3)
                    visit(child, depth + 1);
            }
        };
        const view = this._view();
        if (view)
            visit(view, 0);
        for (const group of [...this._groups.keys()]) {
            if (!found.has(group)) {
                this._forget(group);
                this._groups.delete(group);
            }
        }
        for (const group of found) {
            if (this._groups.has(group))
                continue;
            this._groups.set(group, null);
            const refresh = () => this._refresh(group);
            const source = group.source;
            group.connectObject('notify::expanded', refresh, 'destroy', () => {
                source?.disconnectObject(this);
                this._groups?.delete(group);
            }, this);
            group.source?.connectObject('notification-added', refresh, 'notification-removed', refresh, this);
            this._refresh(group);
        }
    }

    _forget(group) {
        group.disconnectObject(this);
        group.source?.disconnectObject(this);
        const label = this._groups?.get(group);
        if (label && !label._destroyed)
            label.destroy();
    }

    // The top message is the group's first child; its header holds the app's name
    _refresh(group) {
        const old = this._groups.get(group);
        if (old && !old._destroyed)
            old.destroy();
        this._groups.set(group, null);
        const count = group.source?.notifications?.length ?? group._notificationToMessage?.size ?? 0;
        if (group.expanded || count < 2)
            return;
        const message = group.get_first_child()?.child ?? group.get_first_child();
        const title = this._find(message, 'message-source-title');
        const box = title?.get_parent();
        if (!box)
            return;
        const label = new St.Label({style_class: `message-source-title ${LABEL}`, name: LABEL, text: `· ${count}`, y_align: title.y_align});
        label.connect('destroy', () => (label._destroyed = true));
        box.insert_child_above(label, title);
        this._groups.set(group, label);
    }

    _find(actor, styleClass) {
        if (!actor)
            return null;
        if (actor.has_style_class_name?.(styleClass))
            return actor;
        for (const child of actor.get_children()) {
            const found = this._find(child, styleClass);
            if (found)
                return found;
        }
        return null;
    }
}
