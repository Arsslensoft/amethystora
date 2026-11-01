// The force quit list: every running app with what it uses now (processor and memory, read every REFRESH while the
// list is open), an app that does not answer the desktop first and marked so. Picking one and Force quit asks once
// more, naming it, then ends each of its processes that has a window, as GNOME's own Force quit does (Mutter's
// window.kill(): the kernel's SIGKILL, which only reaches this account's processes, else the display protocol's own
// way to end a client). Nothing is ended without that second yes.

import Clutter from 'gi://Clutter';
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import Pango from 'gi://Pango';
import Shell from 'gi://Shell';
import St from 'gi://St';

import {gettext as _} from 'resource:///org/gnome/shell/extensions/extension.js';
import * as ModalDialog from 'resource:///org/gnome/shell/ui/modalDialog.js';

import {inCloseDialog} from './frozen.js';
import {usage, walk} from './usage.js';

// The first reading of the processor's use comes this long after the list opens; then one every REFRESH (ms)
const FIRST = 1000;
const REFRESH = 2000;

// An accelerator as the card's corner shows it: <Super><Control><Shift>Escape is Super+Ctrl+Shift+Esc
export function keyLabel(accelerator) {
    const names = {super: 'Super', control: 'Ctrl', ctrl: 'Ctrl', primary: 'Ctrl', shift: 'Shift', alt: 'Alt'};
    const mods = [...accelerator.matchAll(/<([a-z]+)>/gi)].map(m => names[m[1].toLowerCase()] ?? m[1]);
    let key = accelerator.replace(/<[a-z]+>/gi, '');
    key = {Escape: 'Esc'}[key] ?? key;
    return [...mods, key].join('+');
}

// Whether a window answers the desktop: Mutter's own answer to its last ping
function frozen(app) {
    return app.get_windows().some(window => window.is_alive === false);
}

export class ForceQuitList {
    // keyLabel: the key shown in the corner; closed(): it has gone
    constructor(key, closed) {
        this._closed = closed;
        this._rows = new Map();
        this._selected = null;
        this._sorted = false;
        this._cancellable = new Gio.Cancellable();
        this._dialog = new ModalDialog.ModalDialog({styleClass: 'amethystora-quit-list', destroyOnClose: true});
        this._dialog.connect('closed', () => {
            this._stop();
            this._dialog = null;
            this._closed?.();
        });
        this._dialog.connect('destroy', () => this._stop());
        this._key = key;
        this._buildList();
    }

    open() {
        if (!this._dialog.open()) {
            // Another grab holds the keyboard: nothing opened
            const closed = this._closed;
            this.destroy();
            closed?.();
            return;
        }
        // Each window is asked whether it answers; one that does not is marked when Mutter gives up on it. Each ping has a
        // time of its own: Mutter takes a pong for every ping of that time, so one window answering would answer for all
        let now = global.display.get_current_time_roundtrip();
        for (const app of Shell.AppSystem.get_default().get_running()) {
            for (const window of app.get_windows())
                window.check_alive(now++);
        }
        Shell.AppSystem.get_default().connectObject('app-state-changed', () => this._fill(), this);
        // The keyboard stays here while the list is open: GNOME's own not-responding question takes the key focus into
        // its window whenever Mutter asks it to, even under a modal dialog. Only from that question is it taken back; a
        // dialog opened over the list (a password prompt) keeps it
        global.stage.connectObject('notify::key-focus', () => {
            if (this._dialog?.state === ModalDialog.State.OPENED && inCloseDialog(global.stage.key_focus))
                this._focusTarget()?.grab_key_focus();
        }, this);
        this._read(FIRST);
    }

    get isOpen() {
        return !!this._dialog;
    }

    close() {
        this._dialog?.close();
    }

    destroy() {
        const dialog = this._dialog;
        this._dialog = null;
        this._closed = null;
        this._stop();
        dialog?.popModal();
        dialog?.destroy();
    }

    _stop() {
        this._cancellable?.cancel();
        this._cancellable = null;
        if (this._timer)
            GLib.source_remove(this._timer);
        this._timer = 0;
        Shell.AppSystem.get_default().disconnectObject(this);
        global.stage.disconnectObject(this);
        for (const row of this._rows.values()) {
            for (const window of row.windows)
                window.disconnectObject(this);
        }
        this._rows.clear();
    }

    // --- The list -----------------------------------------------------------------------------------------------

    _buildList() {
        const dialog = this._dialog;
        dialog.contentLayout.destroy_all_children();
        dialog.clearButtons();
        this._rows.clear();
        this._confirming = null;

        const box = new St.BoxLayout({style_class: 'amethystora-quit-box', orientation: Clutter.Orientation.VERTICAL, x_expand: true});
        const head = new St.BoxLayout({style_class: 'amethystora-quit-head'});
        head.add_child(new St.Label({style_class: 'amethystora-quit-title', text: _('Quit an app'), x_expand: true, y_align: Clutter.ActorAlign.END}));
        const key = new St.Label({style_class: 'amethystora-quit-key', text: this._key, y_align: Clutter.ActorAlign.END});
        key.text_direction = Clutter.TextDirection.LTR;
        head.add_child(key);
        box.add_child(head);

        box.add_child(this._line('', _('App'), _('CPU'), _('Memory'), 'amethystora-quit-columns'));
        this._list = new St.BoxLayout({style_class: 'amethystora-quit-rows', orientation: Clutter.Orientation.VERTICAL, x_expand: true});
        const scroll = new St.ScrollView({style_class: 'amethystora-quit-scroll', child: this._list, x_expand: true,
            hscrollbar_policy: St.PolicyType.NEVER, vscrollbar_policy: St.PolicyType.AUTOMATIC});
        box.add_child(scroll);
        // Said instead of an empty list
        this._none = new St.Label({style_class: 'amethystora-quit-text', text: _('No app is open.'), x_align: Clutter.ActorAlign.CENTER});
        box.add_child(this._none);
        dialog.contentLayout.add_child(box);

        const buttons = dialog.buttonLayout;
        buttons.layout_manager.homogeneous = false;
        buttons.add_style_class_name('amethystora-quit-buttons');
        this._spacer = new St.Widget({x_expand: true});
        buttons.add_child(this._spacer);
        this._button(dialog.addButton({label: _('Cancel'), action: () => this.close(), key: Clutter.KEY_Escape}));
        this._quit = this._button(dialog.addButton({label: _('Force quit'), action: () => this._confirm()}), 'amethystora-quit-bad');
        this._fill();
    }

    // Where the keyboard goes back to: the question's Force quit, else the app picked
    _focusTarget() {
        if (this._confirming)
            return this._confirmButton ?? null;
        return this._rows.get(this._selected)?.actor ?? this._quit ?? null;
    }

    _button(button, style = '') {
        button.x_expand = false;
        button.add_style_class_name('amethystora-quit-button');
        if (style)
            button.add_style_class_name(style);
        return button;
    }

    // A row's four cells: the icon (or nothing), the name, the processor's use, the memory
    _line(icon, name, cpu, memory, style) {
        const line = new St.BoxLayout({style_class: `amethystora-quit-line ${style}`, x_expand: true});
        const iconBin = new St.Bin({style_class: 'amethystora-quit-icon', child: icon || null});
        line.add_child(iconBin);
        const nameBox = new St.BoxLayout({style_class: 'amethystora-quit-name', x_expand: true, y_align: Clutter.ActorAlign.CENTER});
        const nameLabel = new St.Label({text: name, y_align: Clutter.ActorAlign.CENTER});
        nameLabel.clutter_text.use_markup = false;
        nameLabel.clutter_text.ellipsize = Pango.EllipsizeMode.END;
        nameBox.add_child(nameLabel);
        line.add_child(nameBox);
        const cpuLabel = new St.Label({style_class: 'amethystora-quit-number amethystora-quit-cpu', text: cpu, y_align: Clutter.ActorAlign.CENTER});
        const memoryLabel = new St.Label({style_class: 'amethystora-quit-number amethystora-quit-memory', text: memory, y_align: Clutter.ActorAlign.CENTER});
        line.add_child(cpuLabel);
        line.add_child(memoryLabel);
        return Object.assign(line, {_quit: {nameBox, nameLabel, cpuLabel, memoryLabel}});
    }

    // The running apps, kept in the order they were sorted in once their use is known; a new one goes last
    _fill() {
        if (!this._dialog || this._confirming)
            return;
        const running = Shell.AppSystem.get_default().get_running().filter(app => app.get_windows().length > 0);
        const ids = new Set(running.map(app => app.get_id()));
        for (const [id, row] of this._rows) {
            if (ids.has(id))
                continue;
            for (const window of row.windows)
                window.disconnectObject(this);
            row.actor.destroy();
            this._rows.delete(id);
        }
        for (const app of running) {
            const id = app.get_id();
            if (this._rows.has(id))
                continue;
            const line = this._line(app.create_icon_texture(24), this._name(app), '…', '…', '');
            const actor = new St.Button({style_class: 'amethystora-quit-row', child: line, can_focus: true, x_expand: true,
                accessible_name: app.get_name()});
            actor.connect('clicked', () => this._select(id));
            actor.connect('key-focus-in', () => this._select(id));
            this._list.add_child(actor);
            const windows = app.get_windows();
            for (const window of windows)
                window.connectObject('notify::is-alive', () => this._mark(id), this);
            this._rows.set(id, {app, actor, line, windows, use: null});
            this._mark(id);
        }
        if (!this._selected || !this._rows.has(this._selected))
            this._select(this._order()[0] ?? null);
        this._none.visible = this._rows.size === 0;
        this._updateQuit();
    }

    // An app's name, and for a terminal the title of its latest window, which says what runs in it
    _name(app) {
        const name = app.get_name() ?? '';
        const terminal = app.get_app_info()?.get_categories()?.split(';').includes('TerminalEmulator');
        const title = terminal ? app.get_windows()[0]?.get_title() : null;
        return title && title !== name ? `${name} · ${title}` : name;
    }

    _mark(id) {
        const row = this._rows.get(id);
        if (!row || this._confirming)
            return;
        const isFrozen = frozen(row.app);
        if (isFrozen && !row.badge) {
            row.badge = new St.Label({style_class: 'amethystora-quit-frozen-badge', text: _('Not responding'), y_align: Clutter.ActorAlign.CENTER});
            row.line._quit.nameBox.add_child(row.badge);
        } else if (!isFrozen && row.badge) {
            row.badge.destroy();
            row.badge = null;
        }
        // A frozen app comes first, whenever it is found to be
        if (isFrozen)
            this._list.set_child_at_index(row.actor, 0);
    }

    // Frozen first, then by processor use, then by name
    _order() {
        return [...this._rows.entries()].sort(([, a], [, b]) => (frozen(b.app) - frozen(a.app)) ||
            ((b.use?.cpu ?? 0) - (a.use?.cpu ?? 0)) || a.app.get_name().localeCompare(b.app.get_name())).map(([id]) => id);
    }

    _select(id) {
        this._selected = id;
        for (const [rowId, row] of this._rows) {
            if (rowId === id)
                row.actor.add_style_pseudo_class('checked');
            else
                row.actor.remove_style_pseudo_class('checked');
        }
        this._updateQuit();
    }

    _updateQuit() {
        const row = this._rows.get(this._selected);
        if (!this._quit)
            return;
        this._quit.label = row ? _('Force quit %s').format(row.app.get_name()) : _('Force quit');
        this._quit.reactive = !!row;
        this._quit.visible = !!row;
    }

    // Reads every process after `delay`, again every REFRESH while the list is open
    _read(delay) {
        if (this._timer)
            GLib.source_remove(this._timer);
        this._timer = GLib.timeout_add(GLib.PRIORITY_DEFAULT, delay, () => {
            this._timer = 0;
            this._sample();
            return GLib.SOURCE_REMOVE;
        });
    }

    async _sample() {
        const cancellable = this._cancellable;
        if (!cancellable)
            return;
        const now = await walk(cancellable);
        // While the second question shows, the readings stop; Back starts them again
        if (!now || cancellable.is_cancelled() || !this._dialog || this._confirming)
            return;
        const before = this._walked ?? null;
        this._walked = now;
        for (const row of this._rows.values()) {
            const pids = [...new Set(row.app.get_windows().map(window => window.get_pid()).filter(pid => pid > 0))];
            row.use = usage(before, now, pids);
            row.line._quit.cpuLabel.text = row.use.cpu === null ? '…' : `${Math.round(row.use.cpu)}%`;
            row.line._quit.memoryLabel.text = GLib.format_size(row.use.memory);
        }
        // Sorted once, when the processor's use is first known: rows that moved under the pointer would be picked wrongly
        if (before && !this._sorted) {
            this._sorted = true;
            this._order().forEach((id, index) => this._list.set_child_at_index(this._rows.get(id).actor, index));
            this._select(this._order()[0] ?? this._selected);
        }
        this._read(REFRESH);
    }

    // --- Asking once more ---------------------------------------------------------------------------------------

    _confirm() {
        const row = this._rows.get(this._selected);
        if (!row || !this._dialog)
            return;
        const app = row.app;
        this._confirming = app;
        if (this._timer)
            GLib.source_remove(this._timer);
        this._timer = 0;
        const dialog = this._dialog;
        dialog.contentLayout.destroy_all_children();
        dialog.clearButtons();
        const box = new St.BoxLayout({style_class: 'amethystora-quit-box amethystora-quit-confirm', orientation: Clutter.Orientation.VERTICAL, x_expand: true});
        const icon = new St.Bin({style_class: 'amethystora-quit-confirm-icon', child: app.create_icon_texture(42), x_align: Clutter.ActorAlign.CENTER});
        box.add_child(icon);
        const title = new St.Label({style_class: 'amethystora-quit-title', text: _('Force quit %s?').format(app.get_name()), x_align: Clutter.ActorAlign.CENTER});
        title.clutter_text.use_markup = false;
        box.add_child(title);
        const text = new St.Label({style_class: 'amethystora-quit-text', text: _('It ends at once. Anything not saved in it is lost.'), x_align: Clutter.ActorAlign.CENTER});
        text.clutter_text.line_wrap = true;
        box.add_child(text);
        dialog.contentLayout.add_child(box);
        dialog.buttonLayout.add_child(new St.Widget({x_expand: true}));
        this._button(dialog.addButton({label: _('Back'), action: () => this._back(), key: Clutter.KEY_Escape}));
        const quit = this._button(dialog.addButton({label: _('Force quit'), action: () => this._kill(app), default: true}), 'amethystora-quit-bad');
        this._confirmButton = quit;
        quit.grab_key_focus();
    }

    _back() {
        if (!this._dialog)
            return;
        // The rows are made again, without their use until the next reading
        for (const row of this._rows.values()) {
            for (const window of row.windows)
                window.disconnectObject(this);
        }
        this._buildList();
        this._sorted = false;
        this._read(FIRST);
        // The question's button had the keyboard and is gone with it: back to the list, where Escape is Cancel again
        this._focusTarget()?.grab_key_focus();
    }

    // Each process of the app that has a window, once: as GNOME's own Force quit ends one
    _kill(app) {
        const done = new Set();
        for (const window of app.get_windows()) {
            const pid = window.get_pid();
            if (done.has(pid))
                continue;
            done.add(pid);
            window.kill();
        }
        this.close();
    }

    // For the suites
    get state() {
        return {
            open: !!this._dialog, confirming: this._confirming?.get_id() ?? null, selected: this._selected,
            // The rows are gone while the second question shows
            rows: this._confirming ? [] : this._order().map(id => {
                const row = this._rows.get(id);
                return {id, name: row.line._quit.nameLabel.text, cpu: row.line._quit.cpuLabel.text, memory: row.line._quit.memoryLabel.text,
                    frozen: !!row.badge};
            }),
        };
    }
}
