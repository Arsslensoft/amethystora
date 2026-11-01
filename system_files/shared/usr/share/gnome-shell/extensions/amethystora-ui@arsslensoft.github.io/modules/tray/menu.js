// A tray item's menu, which its program describes over D-Bus (com.canonical.dbusmenu) and the shell draws as an
// ordinary panel menu: entries, separators, check and radio marks, icons, and submenus that open in place. Entries
// that stay when the program changes its menu are kept and moved rather than made again, so that an open menu does
// not flicker; a long menu is put together a few entries at a time, submenus included, so that it never holds up the
// shell. What the program sends is read as untrusted: only the properties below are kept, each in the one type the
// protocol gives it, and pictures and labels have a ceiling.

import Clutter from 'gi://Clutter';
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import St from 'gi://St';

import * as PopupMenu from 'resource:///org/gnome/shell/ui/popupMenu.js';

import {MENU_IFACE, call, eventTime, isCancelled} from './dbus.js';
import {pngContent} from './icons.js';

// Milliseconds of building per turn of the main loop, for a menu of hundreds of entries
const BUILD_BUDGET = 4;
const MENU_ICON_SIZE = 16;
// An entry's picture is a PNG of a few kilobytes: one beyond these is not drawn (the shell would have to unpack it, and
// a program can give every entry one), and a label is cut to this many characters
const ICON_BYTES = 64 * 1024;
const ICON_SIDE = 256;
const LABEL_LENGTH = 512;

// The layout properties that are read, each with the type string it has in the dbusmenu protocol
const PROPERTY_TYPES = new Map([
    ['type', 's'], ['label', 's'], ['enabled', 'b'], ['visible', 'b'], ['icon-name', 's'], ['icon-data', 'ay'],
    ['toggle-type', 's'], ['toggle-state', 'i'], ['children-display', 's'],
]);

// What an entry is, by the properties that decide which kind of menu row it needs
function kindOf(props) {
    if (props.type === 'separator')
        return 'separator';
    return props['children-display'] === 'submenu' ? 'submenu' : 'entry';
}

// The text of a label, by the dbusmenu rules: "__" shows one underscore, and the first other underscore before a
// character marks the access key and is not shown. A label has one access key, so any later underscore is text.
function labelText(label) {
    if (typeof label !== 'string')
        return '';
    let marked = false;
    return label.slice(0, LABEL_LENGTH).replace(/__|_(?=.)/gs, mark => {
        if (mark === '__')
            return '_';
        if (marked)
            return mark;
        marked = true;
        return '';
    });
}

// A dictionary of properties as an object. A name the tray does not read, or a value of another type than the
// protocol gives it, is left out as if the program had not sent it, so that nothing later has to ask what it holds.
function unpackProps(dict) {
    const props = {};
    for (let i = 0; i < dict.n_children(); i++) {
        const entry = dict.get_child_value(i);
        const name = entry.get_child_value(0).get_string()[0];
        const value = entry.get_child_value(1).get_variant();
        if (value.get_type_string() !== PROPERTY_TYPES.get(name))
            continue;
        if (name !== 'icon-data')
            props[name] = value.recursiveUnpack();
        else if (value.get_size() <= ICON_BYTES)
            props[name] = value.get_data_as_bytes();
    }
    return props;
}

// One node of a layout, (ia{sv}av), with its children
function parseNode(variant) {
    const children = variant.get_child_value(2);
    const node = {
        id: variant.get_child_value(0).get_int32(),
        props: unpackProps(variant.get_child_value(1)),
        children: [],
    };
    for (let i = 0; i < children.n_children(); i++) {
        const child = children.get_child_value(i).get_variant();
        if (child.get_type_string() === '(ia{sv}av)')
            node.children.push(parseNode(child));
    }
    return node;
}

// The picture of an entry: its icon's name, else its PNG, which is unpacked at the size it is drawn at and not at all
// when it says it is bigger than a menu icon could need. A PNG that was unpacked once is kept on the entry for as long
// as its bytes stay the same, since every layout update goes over every entry again.
function entryIcon(entry) {
    const {props} = entry;
    const name = props['icon-name'];
    if (typeof name === 'string' && name !== '')
        return new Gio.ThemedIcon({name});
    const bytes = props['icon-data'];
    if (!bytes || bytes.get_size() === 0)
        return null;
    if (!entry.pictured?.bytes.equal(bytes)) {
        const scale = St.ThemeContext.get_for_stage(global.stage).scale_factor;
        entry.pictured = {bytes, picture: pngContent(bytes, Math.ceil(MENU_ICON_SIZE * scale), ICON_SIDE)};
    }
    return entry.pictured.picture;
}

function ornamentOf(props) {
    const type = props['toggle-type'];
    // Off is 0; on, and the undecided state, are anything else
    const on = (props['toggle-state'] ?? -1) !== 0;
    if (type === 'checkmark')
        return on ? PopupMenu.Ornament.CHECK : PopupMenu.Ornament.NONE;
    if (type === 'radio')
        return on ? PopupMenu.Ornament.DOT : PopupMenu.Ornament.NONE;
    // A plain entry keeps the column, empty, so that its label lines up with those of the entries that tick
    return PopupMenu.Ornament.NONE;
}

export class RemoteMenu {
    /**
     * @param {import('./item.js').Item} item - the tray item whose menu this is
     * @param {string} path - the menu's object path
     * @param {PopupMenu.PopupMenu} menu - the panel button's menu, which this fills
     */
    constructor(item, path, menu) {
        this._item = item;
        this._bus = item.bus;
        this.path = path;
        this._menu = menu;
        this._cancellable = new Gio.Cancellable();
        // Every entry of the layout by its id. The root is the button's menu; an entry's `wanted` are its children
        // in the program's order, `shown` those that have a row, in the order they are shown.
        this._root = {id: 0, kind: 'submenu', props: {}, widget: null, wanted: [], shown: []};
        this._entries = new Map([[0, this._root]]);
        this._aboutToShowWorks = true;
        this._stale = false;
        // The entries that have no row yet, in the order they are made, and the idle source that makes them
        this._queued = new Set();
        this._buildId = 0;

        this._signalId = Gio.DBus.session.signal_subscribe(this._bus, MENU_IFACE, null, path, null,
            Gio.DBusSignalFlags.NONE, (c, s, p, i, signal, params) => this._onSignal(signal, params));
        menu.connectObject(
            'open-state-changed', (m, open) => this._openChanged(this._root, open),
            // The button takes its menu, and every row in it, away before it says it is going
            'destroy', () => {
                this._root.shown = [];
            }, this);

        // Some programs fill their menu only once it is about to be shown for the first time
        this._aboutToShow(0).then(() => this._reload());
    }

    destroy() {
        this._cancellable.cancel();
        Gio.DBus.session.signal_unsubscribe(this._signalId);
        this._menu.disconnectObject(this);
        if (this._buildId)
            GLib.source_remove(this._buildId);
        this._buildId = 0;
        this._queued.clear();
        for (const entry of [...this._root.shown])
            this._unbuild(entry);
        this._entries.clear();
    }

    _onSignal(signal, params) {
        if (signal !== 'LayoutUpdated' && signal !== 'ItemsPropertiesUpdated')
            return;
        // A closed menu that has its entries is brought up to date when it is next opened
        if (!this._menu.isOpen && this._root.shown.length > 0)
            this._stale = true;
        else if (signal === 'LayoutUpdated')
            this._reload();
        else if (params.get_type_string() === '(a(ia{sv})a(ias))')
            this._updateProps(params);
    }

    // Reads the whole layout. A reload asked for while one is under way follows it.
    async _reload() {
        if (this._loading) {
            this._again = true;
            return;
        }
        this._loading = true;
        this._again = false;
        try {
            const reply = await call(this._bus, this.path, MENU_IFACE, 'GetLayout',
                new GLib.Variant('(iias)', [0, -1, []]), '(u(ia{sv}av))', this._cancellable);
            this._apply(parseNode(reply.get_child_value(1)));
        } catch (e) {
            if (!isCancelled(e))
                console.debug(`Amethystora UI: a tray menu could not be read: ${e.message}`);
        } finally {
            this._loading = false;
        }
        if (this._again && !this._cancellable.is_cancelled())
            this._reload();
    }

    // Makes the menu match a layout: what is gone goes, what changed kind or place is made again, the rest is kept
    _apply(layout) {
        const seen = new Set([0]);
        this._root.props = layout.props;
        this._plan(this._root, layout.children, seen);
        for (const [id, entry] of this._entries) {
            if (!seen.has(id)) {
                this._unbuild(entry);
                this._entries.delete(id);
            }
        }
        this._realize(this._root);
    }

    _plan(parent, nodes, seen) {
        parent.wanted = [];
        for (const node of nodes) {
            // An id given twice is shown once
            if (seen.has(node.id))
                continue;
            seen.add(node.id);
            const kind = kindOf(node.props);
            let entry = this._entries.get(node.id);
            if (entry && (entry.kind !== kind || entry.parent !== parent)) {
                this._unbuild(entry);
                entry.kind = kind;
                entry.parent = parent;
            }
            if (!entry) {
                entry = {id: node.id, kind, parent, widget: null, wanted: [], shown: []};
                this._entries.set(node.id, entry);
            }
            entry.props = node.props;
            parent.wanted.push(entry);
            this._plan(entry, node.children, seen);
        }
    }

    // Brings a menu's rows in line with its wanted entries: kept rows are updated in place and moved into the
    // program's order, new rows, in a submenu as much as in the menu, are queued and made at idle. A submenu that
    // changed place is made again, since its open part is a second actor that moving the row leaves behind.
    _realize(parent) {
        let kept = parent.wanted.filter(entry => entry.widget);
        if (kept.some((entry, i) => parent.shown[i] !== entry)) {
            const shown = [...parent.shown];
            for (const entry of kept.filter((e, i) => e.kind === 'submenu' && shown.indexOf(e) !== i))
                this._unbuild(entry);
            kept = kept.filter(entry => entry.widget);
            const menu = parent === this._root ? this._menu : parent.widget.menu;
            kept.forEach((entry, i) => menu.moveMenuItem(entry.widget, i));
            parent.shown = kept;
        }
        for (const entry of parent.wanted) {
            if (entry.widget) {
                this._update(entry);
                if (entry.kind === 'submenu')
                    this._realize(entry);
            } else {
                this._queued.add(entry);
            }
        }
        if (this._queued.size > 0)
            this._queueBuild();
    }

    _queueBuild() {
        if (this._buildId)
            return;
        // Sooner while somebody is looking at the menu
        const priority = this._menu.isOpen ? GLib.PRIORITY_DEFAULT : GLib.PRIORITY_LOW;
        this._buildId = GLib.idle_add(priority, () => {
            let more = false;
            try {
                more = this._buildSome();
            } finally {
                // A throw ends the source: the next queued entry must be free to start another
                if (!more)
                    this._buildId = 0;
            }
            return more ? GLib.SOURCE_CONTINUE : GLib.SOURCE_REMOVE;
        });
    }

    // Makes queued rows until the turn's time is used up; whether any are left. An entry that went away while it
    // waited, or whose submenu has no row, is passed over (its submenu queues it again when it is made).
    _buildSome() {
        const until = GLib.get_monotonic_time() + BUILD_BUDGET * 1000;
        while (this._queued.size > 0) {
            if (GLib.get_monotonic_time() > until)
                return true;
            const [entry] = this._queued;
            this._queued.delete(entry);
            const placed = entry.parent === this._root || entry.parent.widget;
            if (entry.widget || this._entries.get(entry.id) !== entry || !placed)
                continue;
            // One entry that cannot be drawn costs its own row only
            try {
                this._build(entry);
            } catch (e) {
                console.debug(`Amethystora UI: a tray menu entry could not be drawn: ${e.message}`);
            }
        }
        return false;
    }

    _build(entry) {
        const parent = entry.parent;
        const menu = parent === this._root ? this._menu : parent.widget.menu;
        // After the entries before it that have their row
        const position = parent.wanted.slice(0, parent.wanted.indexOf(entry)).filter(e => e.widget).length;

        if (entry.kind === 'separator') {
            entry.widget = new PopupMenu.PopupSeparatorMenuItem();
        } else if (entry.kind === 'submenu') {
            // No picture on a submenu's row, and the empty column the plain entries keep, so that its label lines up
            entry.widget = new PopupMenu.PopupSubMenuMenuItem('', false);
            entry.widget.setOrnament(PopupMenu.Ornament.NONE);
            entry.label = entry.widget.label;
            entry.icon = null;
            entry.widget.menu.connect('open-state-changed', (m, open) => this._openChanged(entry, open));
        } else {
            entry.widget = new PopupMenu.PopupBaseMenuItem();
            entry.label = new St.Label({x_expand: true, y_align: Clutter.ActorAlign.CENTER});
            entry.icon = new St.Icon({
                style_class: 'popup-menu-icon',
                icon_size: MENU_ICON_SIZE,
                x_align: Clutter.ActorAlign.END,
            });
            entry.widget.add_child(entry.label);
            entry.widget.add_child(entry.icon);
            entry.widget.label_actor = entry.label;
            entry.widget.connect('activate', (w, event) => this._clicked(entry, event));
        }
        menu.addMenuItem(entry.widget, position);
        parent.shown.splice(position, 0, entry);
        this._update(entry);
        if (entry.kind === 'submenu')
            this._realize(entry);
    }

    // Takes an entry's row away, and with a submenu's row every row inside it
    _unbuild(entry) {
        if (!entry.widget)
            return;
        const forget = e => {
            for (const child of e.shown)
                forget(child);
            e.shown = [];
            e.widget = e.label = e.icon = null;
        };
        const widget = entry.widget;
        entry.parent.shown = entry.parent.shown.filter(e => e !== entry);
        forget(entry);
        widget.destroy();
    }

    _update(entry) {
        const {props, widget} = entry;
        widget.visible = props.visible ?? true;
        if (entry.kind === 'separator')
            return;
        widget.setSensitive(props.enabled ?? true);
        entry.label.text = labelText(props.label);
        if (entry.kind === 'entry') {
            const gicon = entryIcon(entry);
            entry.icon.gicon = gicon;
            entry.icon.visible = gicon !== null;
            widget.setOrnament(ornamentOf(props));
        }
    }

    _updateProps(params) {
        const changed = params.get_child_value(0);
        const removed = params.get_child_value(1);
        const touched = new Set();
        let rebuild = false;
        for (let i = 0; i < changed.n_children(); i++) {
            const child = changed.get_child_value(i);
            const entry = this._entries.get(child.get_child_value(0).get_int32());
            if (!entry || entry === this._root)
                continue;
            Object.assign(entry.props, unpackProps(child.get_child_value(1)));
            touched.add(entry);
        }
        for (let i = 0; i < removed.n_children(); i++) {
            const child = removed.get_child_value(i);
            const entry = this._entries.get(child.get_child_value(0).get_int32());
            if (!entry || entry === this._root)
                continue;
            // A property taken away is back at its default
            for (const name of child.get_child_value(1).get_strv())
                delete entry.props[name];
            touched.add(entry);
        }
        for (const entry of touched) {
            if (kindOf(entry.props) !== entry.kind)
                rebuild = true;
            else if (entry.widget)
                this._update(entry);
        }
        // An entry that turned into another kind is made again in its place
        if (rebuild)
            this._reload();
    }

    _openChanged(entry, open) {
        const time = eventTime();
        if (!open) {
            this._event(entry.id, 'closed', time);
            return;
        }
        this._event(entry.id, 'opened', time);
        // What the person is about to see is made first, and sooner than an idle menu would
        const waiting = entry.wanted.filter(e => !e.widget);
        if (waiting.length > 0)
            this._queued = new Set([...waiting, ...this._queued]);
        if (this._buildId) {
            GLib.source_remove(this._buildId);
            this._buildId = 0;
        }
        if (this._queued.size > 0)
            this._queueBuild();
        if (entry === this._root && this._stale) {
            this._stale = false;
            this._reload();
        }
        this._aboutToShow(entry.id).then(update => {
            if (update)
                this._reload();
        });
    }

    _clicked(entry, event) {
        const time = event?.get_time() ?? 0;
        // With the click's time, the program may raise its window
        if (time)
            this._item.provideToken(time);
        this._event(entry.id, 'clicked', time || eventTime());
    }

    // Events need no answer; one about a menu its program has taken away already is of no interest
    _event(id, name, time) {
        call(this._bus, this.path, MENU_IFACE, 'Event',
            new GLib.Variant('(isvu)', [id, name, new GLib.Variant('i', 0), time]), null, this._cancellable)
            .catch(() => {});
    }

    // Whether the program wants the menu read again before it is shown. An answer without a value (some programs
    // answer so) counts as yes; a program that does not know the call is not asked again.
    async _aboutToShow(id) {
        if (!this._aboutToShowWorks)
            return false;
        try {
            const reply = await call(this._bus, this.path, MENU_IFACE, 'AboutToShow',
                new GLib.Variant('(i)', [id]), null, this._cancellable);
            const value = reply.n_children() > 0 ? reply.get_child_value(0) : null;
            return !value || value.get_type_string() !== 'b' || value.get_boolean();
        } catch (e) {
            if (!isCancelled(e))
                this._aboutToShowWorks = false;
            return false;
        }
    }
}
