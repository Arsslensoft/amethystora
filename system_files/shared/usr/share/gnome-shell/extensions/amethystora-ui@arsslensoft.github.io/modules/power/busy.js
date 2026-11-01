// A busy app: when the whole processor has been at 70% or more for two minutes, a chip beside the meters names the
// app that used the most, and a click says more, with Show it, Not now and Quit. It rides the meters' own reading,
// every five seconds, and reads nothing for it until the two minutes are up: then the processes are looked at twice,
// at that reading and the next, since a share needs two looks. Said once per app per hour, never while a window is
// fullscreen in front (a game), never for anything that is not an app. The chip goes when the processor calms down.
//
// Quit asks the app to quit, as its own menu would (GNOME's request_quit), never a signal.

import Clutter from 'gi://Clutter';
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import GObject from 'gi://GObject';
import Shell from 'gi://Shell';
import St from 'gi://St';

import {gettext as _} from 'resource:///org/gnome/shell/extensions/extension.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import * as PanelMenu from 'resource:///org/gnome/shell/ui/panelMenu.js';
import * as PopupMenu from 'resource:///org/gnome/shell/ui/popupMenu.js';

import {plain} from '../pill/text.js';
import {Walker, usedBetween} from './apps.js';

const ROLE = 'amethystora-power-busy';
// The load that counts as busy, how many readings in a row (5 s each: two minutes), and how long an app is not named
// again (ms)
const BUSY = 0.7;
const READINGS = 24;
const AGAIN = 3600 * 1000;

// When each app was last named, kept across the lock: id -> ms
const named = new Map();

const Chip = GObject.registerClass(
class AmethystoraPowerBusyChip extends PanelMenu.Button {
    _init(busy) {
        super._init(0.5, _('Busy app'));
        this.add_style_class_name('amethystora-power-busy-button');
        this.menu.actor.add_style_class_name('amethystora-power-busy-menu');
        const box = new St.BoxLayout({style_class: 'amethystora-power-chip', y_align: Clutter.ActorAlign.CENTER});
        box.add_child(new St.Icon({icon_name: 'speedometer-symbolic', fallback_icon_name: 'utilities-system-monitor-symbolic',
            icon_size: 13, style_class: 'amethystora-power-chip-icon', y_align: Clutter.ActorAlign.CENTER}));
        this._label = new St.Label({y_align: Clutter.ActorAlign.CENTER});
        box.add_child(this._label);
        this.add_child(box);
        this._card = new PopupMenu.PopupBaseMenuItem({reactive: false, can_focus: false,
            style_class: 'amethystora-power-busy-item'});
        this._body = new St.BoxLayout({style_class: 'amethystora-power-busy-card', orientation: Clutter.Orientation.VERTICAL,
            x_expand: true});
        this._card.add_child(this._body);
        this.menu.addMenuItem(this._card);
        this._busy = busy;
        this.menu.connect('open-state-changed', (menu, open) => {
            if (!open)
                busy._cardClosed();
        });
    }

    fill(found) {
        this._found = found;
        this._label.text = _('%s is busy').format(found.name);
        this.accessible_name = this._label.text;
        this._body.destroy_all_children();
        const title = new St.Label({style_class: 'amethystora-power-busy-title',
            text: _('%s has kept the processor busy for 2 minutes').format(found.name)});
        title.clutter_text.line_wrap = true;
        this._body.add_child(title);
        let words = _('It may be working, or stuck.');
        if (found.terminal && found.process)
            words = _('Mostly “%s”, running in it. It may be working, or stuck.').format(found.process);
        else if (found.window)
            words = _('Mostly “%s”. It may be working, or stuck.').format(found.window);
        const text = new St.Label({style_class: 'amethystora-power-busy-text', text: words});
        text.clutter_text.line_wrap = true;
        this._body.add_child(text);
        const buttons = new St.BoxLayout({style_class: 'amethystora-power-busy-buttons'});
        const button = (label, run, extra = '') => {
            const b = new St.Button({style_class: `amethystora-power-busy-button-action ${extra}`, label, can_focus: true,
                accessible_name: label});
            b.clutter_text?.set_use_markup?.(false);
            b.connect('clicked', () => {
                this.menu.close();
                run();
            });
            buttons.add_child(b);
        };
        button(_('Show it'), () => this._busy.showApp());
        button(_('Not now'), () => this._busy.dismiss());
        // A terminal is not quit for what runs in it: that would close every window it has
        if (!found.terminal)
            button(_('Quit %s').format(found.name), () => this._busy.quitApp(), 'amethystora-power-busy-quit');
        this._body.add_child(buttons);
        const foot = new St.Label({style_class: 'amethystora-power-busy-foot',
            text: _('Said once per app per hour. Never for a game in front.')});
        foot.clutter_text.line_wrap = true;
        this._body.add_child(foot);
    }
});

export class Busy {
    constructor(meters, glass) {
        this._meters = meters;
        this._glass = glass;
    }

    enable() {
        if (!this._meters?.listen)
            return;
        this._readings = 0;
        this._walker = new Walker();
        this._unlisten = this._meters.listen(reading => this._reading(reading));
    }

    disable() {
        if (this._idle)
            GLib.source_remove(this._idle);
        this._idle = 0;
        this._unlisten?.();
        this._unlisten = null;
        this._reset();
        this._walker?.clear();
        this._walker = null;
    }

    _reset() {
        this._readings = 0;
        this._cancellable?.cancel();
        this._cancellable = null;
        this._first = null;
        this._decided = false;
        this._calm = false;
        this._hideChip();
    }

    // The card closed: if the processor calmed down while it was open, the chip goes now
    _cardClosed() {
        if (!this._calm || this._idle)
            return;
        // Once the menu has finished closing, not in the middle of it
        this._idle = GLib.idle_add(GLib.PRIORITY_DEFAULT, () => {
            this._idle = 0;
            if (this._calm)
                this._reset();
            return GLib.SOURCE_REMOVE;
        });
    }

    _reading({load}) {
        load = this._testLoad ?? load;
        if (load < BUSY) {
            // The spell is over; an open card stays until it is closed, and then goes (_cardClosed)
            if (this._chip?.menu.isOpen) {
                this._calm = true;
                this._readings = 0;
            } else if (this._readings > 0 || this._decided || this._chip) {
                this._reset();
            }
            return;
        }
        this._calm = false;
        this._readings++;
        if (this._readings < READINGS || this._decided || this._walking)
            return;
        this._look();
    }

    // The first look at two minutes, the second at the next reading, and then a decision for the whole spell
    async _look() {
        this._walking = true;
        this._cancellable ??= new Gio.Cancellable();
        const cancellable = this._cancellable;
        try {
            const walk = await this._walker.walk(cancellable);
            if (cancellable.is_cancelled())
                return;
            if (!this._first) {
                this._first = walk;
                return;
            }
            this._decided = true;
            this._decide(usedBetween(this._first, walk));
        } catch (e) {
            if (!e.matches?.(Gio.IOErrorEnum, Gio.IOErrorEnum.CANCELLED))
                console.warn(`Amethystora UI: the busy app could not be found: ${e.message}`);
        } finally {
            this._walking = false;
        }
    }

    _decide(used) {
        const top = used.list[0];
        if (!top?.app)
            return;
        // Never over a game (or anything) fullscreen in front
        if (global.display.focus_window?.is_fullscreen())
            return;
        const id = top.app.get_id();
        if (Date.now() - (named.get(id) ?? 0) < AGAIN)
            return;
        named.set(id, Date.now());
        const window = top.app.get_windows()[0]?.get_title?.();
        const terminal = (top.app.get_app_info?.()?.get_categories?.() ?? '').split(';').includes('TerminalEmulator');
        this.found = {app: top.app, name: plain(top.app.get_name(), 40), window: plain(window ?? '', 80) || null,
            terminal, process: plain(top.top ?? '', 40) || null};
        this._showChip();
    }

    _showChip() {
        if (!this._chip) {
            this._chip = new Chip(this);
            const meter = this._meters.processorButton?.container;
            const box = meter?.get_parent();
            const index = box ? box.get_children().indexOf(meter) : 0;
            Main.panel.addToStatusArea(ROLE, this._chip, Math.max(index, 0), 'right');
            this._glass?.()?.surfacesChanged?.();
        }
        this._chip.fill(this.found);
    }

    _hideChip() {
        if (!this._chip)
            return;
        this._chip.destroy();
        this._chip = null;
        this._glass?.()?.surfacesChanged?.();
    }

    showApp() {
        this.found?.app?.activate();
    }

    dismiss() {
        this._hideChip();
    }

    quitApp() {
        this.found?.app?.request_quit();
        this._hideChip();
    }

    // --- For the suites -------------------------------------------------------------------------------------------

    state() {
        const buttons = this._chip ? this._chip._body.get_children().flatMap(c => c.get_children())
            .filter(c => c instanceof St.Button).map(b => b.label) : [];
        return {readings: this._readings ?? 0, chip: this._chip ? this._chip._label.text : null, decided: !!this._decided,
            open: !!this._chip?.menu.isOpen, buttons};
    }

    // As if the meters read `load` `times` times in a row; the meters' own readings read it too until null is fed
    feedForTest(load, times = 1) {
        this._testLoad = load;
        for (let i = 0; i < times; i++)
            this._reading({load});
        return this.state();
    }

    // Names an app as the decision would (by its launcher id), without waiting two minutes; again: as within the hour
    nameForTest(id, again = false) {
        const app = Shell.AppSystem.get_default().lookup_app(id);
        if (!app)
            return false;
        if (!again)
            named.delete(app.get_id());
        this._decide({list: [{app, ticks: 1, name: app.get_name()}], total: 1});
        return !!this._chip;
    }
}
