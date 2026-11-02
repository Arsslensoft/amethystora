// One guide being walked: a ring round the real thing and a card that says what to do. It presses nothing, takes no grab
// and no focus: the ring takes no clicks, and only the card's own buttons do anything, one of which may start the app a
// step is about (the person's click). Whatever cannot be found says the step's words and leads back to the manual.

import Clutter from 'gi://Clutter';
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import Pango from 'gi://Pango';
import Shell from 'gi://Shell';
import St from 'gi://St';

import {gettext as _} from 'resource:///org/gnome/shell/extensions/extension.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import * as Util from 'resource:///org/gnome/shell/misc/util.js';

import {TARGETS} from './targets.js';

// What every Amethystora app answers on its own bus name (apps/common/src/where_is.rs)
const WHERE_PATH = '/org/amethystora/Where';
const WHERE_INTERFACE = 'org.amethystora.Where';
const MANUAL = 'amethystora-manual';
// A step in an app is looked for this often, for this long: its page may need opening first
const FIND_EVERY = 1500;
const FIND_FOR = 30000;
// Once found, it is asked again this often for as long as the step is shown
const FIND_AGAIN = 2500;
// A menu slides open after it is mapped, and the ring is put right once it has
const SETTLED = 300;
const MARGIN = 6;
const CARD_GAP = 14;
const EDGE = 12;
const MOTION = 180;

export class Walk {
    constructor(guide, onEnd) {
        this._guide = guide;
        this._onEnd = onEnd;
        this._at = -1;
        this._cancelled = false;
        this._ring = new St.Widget({style_class: 'amethystora-guide-ring', reactive: false, visible: false});
        Main.layoutManager.addTopChrome(this._ring);
        this._card = new St.BoxLayout({style_class: 'amethystora-guide-card', orientation: Clutter.Orientation.VERTICAL, reactive: true, opacity: 0});
        this._title = new St.Label({style_class: 'amethystora-guide-title', text: guide.title});
        this._step = new St.Label({style_class: 'amethystora-guide-step'});
        this._step.clutter_text.use_markup = false;
        this._step.clutter_text.line_wrap = true;
        this._step.clutter_text.ellipsize = Pango.EllipsizeMode.NONE;
        this._buttons = new St.BoxLayout({style_class: 'amethystora-guide-buttons', x_align: Clutter.ActorAlign.END});
        for (const child of [this._title, this._step, this._buttons])
            this._card.add_child(child);
        Main.layoutManager.addTopChrome(this._card);
        // Gone with the session's normal mode: the extension's disable ends the walk at a lock
        Main.overview.connectObject('showing', () => this._hideRing(), 'hidden', () => this._locate(), this);
        this._go(0);
    }

    // back: the Manual was put out of the way when the guide began, and comes back whichever way the guide ends (done, Stop, a
    // lock, the key turned off). Not when another guide replaces this one: that one brings it back when it ends.
    end(back = true) {
        if (this._cancelled)
            return;
        this._cancelled = true;
        this._leaveStep();
        Main.overview.disconnectObject(this);
        this._ring.destroy();
        this._card.destroy();
        if (back)
            this._bringManualBack();
        this._onEnd();
    }

    // The shell is the compositor: it can bring a window back that a client could not unminimize for itself. Only a window that
    // is minimized, so that a Manual the person has already come back to is left where it is
    _bringManualBack() {
        try {
            const manual = Shell.AppSystem.get_default().lookup_app(`${MANUAL}.desktop`);
            for (const window of manual?.get_windows() ?? []) {
                if (window.minimized) {
                    window.unminimize();
                    Main.activateWindow(window);
                }
            }
        } catch (e) {
            console.warn(`Amethystora UI: bringing the Manual back: ${e.message}`);
        }
    }

    _go(index) {
        this._leaveStep();
        this._at = index;
        const step = this._guide.steps[index];
        const count = this._guide.steps.length;
        this._title.text = this._guide.title;
        this._step.text = count > 1
            ? _('Step {n} of {count}: {words}').replace('{n}', String(index + 1)).replace('{count}', String(count)).replace('{words}', step.say)
            : step.say;
        this._fillButtons(step, null);
        this._raise();
        this._place(null);
        this._card.ease({opacity: 255, duration: MOTION, mode: Clutter.AnimationMode.EASE_OUT_QUAD});
        // A step that follows a menu opening: the menu is still sliding, and raises itself over the ring and card as it does
        if (step.shell)
            this._locateSoon();
        else
            this._locate();
    }

    _leaveStep() {
        this._cancellable?.cancel();
        this._cancellable = null;
        if (this._findTimer)
            GLib.source_remove(this._findTimer);
        this._findTimer = 0;
        if (this._settleTimer)
            GLib.source_remove(this._settleTimer);
        this._settleTimer = 0;
        if (this._laterId)
            GLib.source_remove(this._laterId);
        this._laterId = 0;
        this._watched?.disconnectObject(this);
        this._watched = null;
        this._next?.disconnectObject(this);
        this._next = null;
        this._window?.disconnectObject(this);
        this._window = null;
        this._box = null;
        Shell.WindowTracker.get_default().disconnectObject(this);
        Shell.AppSystem.get_default().disconnectObject(this);
    }

    _fillButtons(step, offerApp) {
        this._buttons.destroy_all_children();
        this._addButton(_('Back to the manual'), false, () => {
            const {page, section} = this._guide;
            this.end();
            Util.spawn([MANUAL, section ? `${page}#${section}` : page]);
        });
        if (offerApp) {
            this._addButton(_('Open {app}').replace('{app}', offerApp.get_name()), true, () => offerApp.activate());
            return;
        }
        const last = this._at === this._guide.steps.length - 1;
        this._addButton(last ? _('Done') : _('Next'), true, () => {
            if (last)
                this.end();
            else
                this._go(this._at + 1);
        });
    }

    _addButton(label, primary, action) {
        const button = new St.Button({
            style_class: `amethystora-guide-button${primary ? ' primary' : ''}`,
            label, can_focus: true,
        });
        button.connect('clicked', action);
        this._buttons.add_child(button);
    }

    // ---- Finding it ----

    _locate() {
        if (this._cancelled)
            return;
        const step = this._guide.steps[this._at];
        if (step.shell)
            this._locateInShell(step);
        else if (step.app)
            this._locateInApp(step);
        else
            this._hideRing();
    }

    _resolve(name) {
        try {
            return TARGETS[name]?.find() ?? null;
        } catch (e) {
            console.warn(`Amethystora UI: a guide's target ${name}: ${e.message}`);
            return null;
        }
    }

    _locateInShell(step) {
        const target = TARGETS[step.shell];
        const actor = this._resolve(step.shell);
        // A toggle GNOME hides is not there to point at: no Bluetooth adapter, no Wi-Fi card
        if (!actor?.visible) {
            this._notFound();
            return;
        }
        // While it is not on the screen, the way to it: the menu it is in
        const shown = actor.mapped ? actor : (target.via ? this._resolve(target.via) : null);
        if (this._watched !== actor) {
            this._watched?.disconnectObject(this);
            this._watched = actor;
            actor.connectObject('notify::mapped', () => this._later(() => this._locateSoon()), this);
        }
        this._watchNextStep();
        if (!shown?.mapped) {
            this._hideRing();
            return;
        }
        this._ringOn(shown.get_transformed_extents());
    }

    // Once the thing the next step points at is on the screen, this step is done: the person opened the menu
    _watchNextStep() {
        const next = this._guide.steps[this._at + 1];
        if (!next?.shell || this._next)
            return;
        const actor = this._resolve(next.shell);
        if (!actor)
            return;
        this._next = actor;
        actor.connectObject('notify::mapped', () => {
            if (actor.mapped)
                this._later(() => this._go(this._at + 1));
        }, this);
    }

    // A change of `mapped` arrives in the middle of the shell's layout, where nothing may be moved or made
    _later(action) {
        if (this._laterId)
            GLib.source_remove(this._laterId);
        this._laterId = GLib.idle_add(GLib.PRIORITY_DEFAULT, () => {
            this._laterId = 0;
            action();
            return GLib.SOURCE_REMOVE;
        });
    }

    _locateSoon() {
        this._locate();
        if (this._settleTimer)
            GLib.source_remove(this._settleTimer);
        this._settleTimer = GLib.timeout_add(GLib.PRIORITY_DEFAULT, SETTLED, () => {
            this._settleTimer = 0;
            this._locate();
            return GLib.SOURCE_REMOVE;
        });
    }

    _locateInApp(step) {
        const apps = Shell.AppSystem.get_default();
        const tracker = Shell.WindowTracker.get_default();
        const app = apps.lookup_app(step.launcher);
        if (!app) {
            this._notFound();
            return;
        }
        // Until the app has the focus, the card offers to open it, and the ring waits
        tracker.disconnectObject(this);
        tracker.connectObject('notify::focus-app', () => this._locate(), this);
        const window = tracker.focus_app === app ? tracker.focus_app.get_windows()[0] ?? null : null;
        if (!window) {
            // The app lost the focus: nothing is drawn about it, and nothing asks it, until it has the focus again
            this._window?.disconnectObject(this);
            this._window = null;
            this._box = null;
            this._askAgain = false;
            this._cancellable?.cancel();
            if (this._findTimer)
                GLib.source_remove(this._findTimer);
            this._findTimer = 0;
            this._hideRing();
            this._fillButtons(step, app);
            this._place(null);
            return;
        }
        this._fillButtons(step, null);
        if (this._window !== window) {
            this._window?.disconnectObject(this);
            this._window = window;
            // A window that moves carries the ring with it; one that changes size is asked again
            window.connectObject('position-changed', () => this._showBox(window), 'size-changed', () => this._askApp(step, window), this);
        }
        this._findUntil = GLib.get_monotonic_time() + FIND_FOR * 1000;
        this._askApp(step, window);
    }

    async _askApp(step, window) {
        // One question at a time: a new position or a page that changed ask again when this answers
        if (this._asking) {
            this._askAgain = true;
            return;
        }
        this._asking = true;
        this._cancellable = new Gio.Cancellable();
        let box = null;
        try {
            const parts = await this._where(window, step.find);
            if (parts[2] > 0 && parts[3] > 0)
                box = parts;
        } catch (e) {
            if (!this._cancelled)
                console.debug(`Amethystora UI: a guide's search: ${e.message}`);
        }
        this._asking = false;
        if (this._cancelled)
            return;
        const again = this._askAgain;
        this._askAgain = false;
        if (this._guide.steps[this._at] !== step || this._window !== window) {
            // Another step asked while this one was still out
            if (again)
                this._locate();
            return;
        }
        if (again) {
            this._askApp(step, window);
            return;
        }
        this._box = box;
        if (box) {
            // Found, and still asked now and then while the step lasts: a page changed or scrolled in the app moves the thing,
            // and the ring would stay round what is no longer there
            this._findUntil = GLib.get_monotonic_time() + FIND_FOR * 1000;
            this._showBox(window);
            this._askLater(step, window, FIND_AGAIN);
        } else if (GLib.get_monotonic_time() < this._findUntil) {
            // Not there (yet): its page may need opening first, so it is looked for again, for a while
            this._hideRing();
            this._askLater(step, window, FIND_EVERY);
        } else {
            this._notFound();
        }
    }

    _askLater(step, window, every) {
        this._findTimer ||= GLib.timeout_add(GLib.PRIORITY_DEFAULT, every, () => {
            this._findTimer = 0;
            if (!this._cancelled && this._window === window)
                this._askApp(step, window);
            return GLib.SOURCE_REMOVE;
        });
    }

    _where(window, name) {
        return new Promise((resolve, reject) => {
            Gio.DBus.session.call(window.get_gtk_application_id() ?? window.get_wm_class(), WHERE_PATH, WHERE_INTERFACE, 'Find',
                new GLib.Variant('(s)', [name]), new GLib.VariantType('(dddd)'), Gio.DBusCallFlags.NO_AUTO_START, 2000, this._cancellable, (bus, result) => {
                    try {
                        resolve(bus.call_finish(result).deepUnpack());
                    } catch (e) {
                        reject(e);
                    }
                });
        });
    }

    // What the app said is from its window's corner; GTK knows nothing of the screen, and the shell places the window. In
    // logical pixels, which a stage in physical pixels has to scale
    _showBox(window) {
        const box = this._box;
        if (!box || this._window !== window)
            return;
        const scale = St.ThemeContext.get_for_stage(global.stage).scale_factor;
        const origin = window.get_buffer_rect();
        this._ringOn({
            get_x: () => origin.x + box[0] * scale,
            get_y: () => origin.y + box[1] * scale,
            get_width: () => box[2] * scale,
            get_height: () => box[3] * scale,
        });
    }

    // ---- Showing it ----

    _notFound() {
        this._hideRing();
        const words = _('I cannot point at this one here. The manual says it in words.');
        const step = this._guide.steps[this._at];
        if (!this._step.text.endsWith(words))
            this._step.text = `${step.say}\n${words}`;
        this._place(null);
    }

    _raise() {
        // Above a menu that opened since: a menu is put at the top of the same group as it opens
        const group = this._ring.get_parent();
        group?.set_child_above_sibling(this._ring, null);
        group?.set_child_above_sibling(this._card, null);
    }

    _hideRing() {
        this._ring.hide();
        this._place(null);
    }

    _ringOn(box) {
        // Nothing is drawn over the overview: it shows the ring again when it is gone
        if (Main.overview.visible) {
            this._hideRing();
            return;
        }
        const [x, y, width, height] = [box.get_x() - MARGIN, box.get_y() - MARGIN, box.get_width() + 2 * MARGIN, box.get_height() + 2 * MARGIN];
        const wasShown = this._ring.visible;
        this._ring.set_size(width, height);
        if (wasShown) {
            this._ring.ease({x, y, duration: MOTION, mode: Clutter.AnimationMode.EASE_OUT_QUAD});
        } else {
            this._ring.set_position(x, y);
            this._ring.opacity = 0;
            this._ring.show();
            this._ring.ease({opacity: 255, duration: MOTION, mode: Clutter.AnimationMode.EASE_OUT_QUAD});
        }
        this._raise();
        this._place({x, y, width, height});
    }

    // Under the ring where it fits, else over it, inside the screen; with no ring, at the top of the main screen
    _place(ring) {
        const within = one => ring && ring.x + ring.width / 2 >= one.x && ring.x + ring.width / 2 < one.x + one.width &&
            ring.y + ring.height / 2 >= one.y && ring.y + ring.height / 2 < one.y + one.height;
        const monitor = Main.layoutManager.monitors.find(within) ?? Main.layoutManager.primaryMonitor;
        const [, width] = this._card.get_preferred_width(-1);
        const [, natural] = this._card.get_preferred_height(width);
        let x = monitor.x + (monitor.width - width) / 2;
        let y = monitor.y + monitor.height * 0.12;
        if (ring) {
            x = ring.x + ring.width / 2 - width / 2;
            y = ring.y + ring.height + CARD_GAP;
            if (y + natural > monitor.y + monitor.height - EDGE)
                y = ring.y - CARD_GAP - natural;
        }
        x = Math.max(monitor.x + EDGE, Math.min(x, monitor.x + monitor.width - width - EDGE));
        y = Math.max(monitor.y + EDGE, Math.min(y, monitor.y + monitor.height - natural - EDGE));
        this._card.set_position(Math.round(x), Math.round(y));
    }
}
