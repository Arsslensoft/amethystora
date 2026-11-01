// Walk-away lock, a security switch (WALK_AWAY in /etc/amethystora/security.conf, off as the image ships; Security's
// Settings page and `ame security walk-away` change it): when every paired phone that was connected has been gone for a
// minute and nobody has touched the computer for a minute, the screen locks. It never unlocks anything, and it locks once
// for each time the phone leaves: after the person unlocks, it waits for a phone to come back and leave again.
//
// The phones are the phone link's, as the connect module follows them through the program's D-Bus contract, in which
// the shell is the module caller the program lets read its phones. The program stopping (the phone link turned off) is
// not a phone leaving. The switch is read through its one reader, amethystora-security-config, only when a phone
// leaves; off, nothing more happens. Nothing runs while no phone is leaving.

import Clutter from 'gi://Clutter';
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import St from 'gi://St';

import {gettext as _} from 'resource:///org/gnome/shell/extensions/extension.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';

const CONFIG = '/usr/libexec/amethystora-security-config';
// How long the phone is gone, and the computer left alone, before the screen locks
const AWAY_SECONDS = 60;
const IDLE_MS = 60 * 1000;
// How long the card shows before the lock
const CARD_MS = 1500;
// gnome-session's inhibitor flag for idle (a video playing, a presentation)
const INHIBIT_IDLE = 8;
// How many times it locked and showed its card in this session: kept across the lock, which stops the module
const told = {locks: 0, cards: 0};

export class Away {
    constructor(ext) {
        this._ext = ext;
        this._ready = 0;
    }

    start() {
        const connect = this._ext.modules?.get('connect');
        if (!connect?.onPhonesChanged)
            return;
        this._connect = connect;
        this._ready = connect.phones().length;
        this._unlisten = connect.onPhonesChanged(() => this._phonesChanged());
    }

    stop() {
        this._unlisten?.();
        this._unlisten = null;
        this._connect = null;
        this._disarm();
        this._card?.destroy();
        this._card = null;
        this._cancellable?.cancel();
        this._cancellable = null;
    }

    _phonesChanged() {
        const ready = this._connect?.phones().length ?? 0;
        const before = this._ready;
        this._ready = ready;
        if (ready > 0) {
            // Back: nothing locks
            this._disarm();
            return;
        }
        // A departure is the last phone going while the program is still there
        if (before > 0 && this._connect?.linked)
            this._departed();
    }

    // Asks the switch, then waits a minute for the phone to come back
    _departed() {
        this._cancellable?.cancel();
        const cancellable = this._cancellable = new Gio.Cancellable();
        let process;
        try {
            process = Gio.Subprocess.new([CONFIG, '--is', 'WALK_AWAY', 'on'], Gio.SubprocessFlags.STDOUT_SILENCE |
                Gio.SubprocessFlags.STDERR_SILENCE);
        } catch {
            return;
        }
        process.wait_check_async(cancellable, (p, result) => {
            let on = false;
            try {
                on = p.wait_check_finish(result);
            } catch {}
            if (cancellable.is_cancelled() || !on || this._ready > 0 || this._timer)
                return;
            this._cancellable = null;
            this._timer = GLib.timeout_add_seconds(GLib.PRIORITY_DEFAULT, AWAY_SECONDS, () => {
                this._timer = 0;
                this._gone();
                return GLib.SOURCE_REMOVE;
            });
        });
    }

    // Gone a minute: lock now if the computer has been left alone a minute, else when it has
    _gone() {
        const monitor = global.backend.get_core_idle_monitor();
        if (monitor.get_idletime() >= IDLE_MS) {
            this._lock();
            return;
        }
        this._monitor = monitor;
        this._watch = monitor.add_idle_watch(IDLE_MS, () => {
            this._monitor?.remove_watch(this._watch);
            this._watch = 0;
            this._monitor = null;
            this._lock();
        });
    }

    _disarm() {
        if (this._timer)
            GLib.source_remove(this._timer);
        this._timer = 0;
        if (this._watch)
            this._monitor?.remove_watch(this._watch);
        this._watch = 0;
        this._monitor = null;
        if (this._lockTimer)
            GLib.source_remove(this._lockTimer);
        this._lockTimer = 0;
        // A phone back during the card takes the card away with the lock
        this._card?.destroy();
        this._card = null;
    }

    // Not while something holds the computer awake (a video, a presentation), as GNOME's own lock waits for it
    _lock() {
        if (this._ready > 0 || Main.sessionMode.isLocked)
            return;
        Gio.DBus.session.call('org.gnome.SessionManager', '/org/gnome/SessionManager', 'org.gnome.SessionManager',
            'IsInhibited', new GLib.Variant('(u)', [INHIBIT_IDLE]), new GLib.VariantType('(b)'),
            Gio.DBusCallFlags.NO_AUTO_START, 2000, null, (bus, result) => {
                let inhibited = false;
                try {
                    [inhibited] = bus.call_finish(result).deepUnpack();
                } catch {}
                if (inhibited || this._ready > 0 || !this._connect)
                    return;
                this._showCard();
                this._lockTimer = GLib.timeout_add(GLib.PRIORITY_DEFAULT, CARD_MS, () => {
                    this._lockTimer = 0;
                    told.locks++;
                    this.lockScreen();
                    this._card?.destroy();
                    this._card = null;
                    return GLib.SOURCE_REMOVE;
                });
            });
    }

    // GNOME's own lock, which honours a locked-down screen lock itself (the suite stands in for it where no display
    // manager runs)
    lockScreen() {
        Main.screenShield?.lock(true);
    }

    _showCard() {
        this._card?.destroy();
        const card = this._card = new St.BoxLayout({
            style_class: 'amethystora-privacy-away',
            orientation: Clutter.Orientation.VERTICAL,
            reactive: false,
        });
        card.add_child(new St.Icon({icon_name: 'phone-symbolic', icon_size: 26, x_align: Clutter.ActorAlign.CENTER}));
        card.add_child(new St.Label({style_class: 'amethystora-privacy-away-title', text: _('Your phone left · locking'),
            x_align: Clutter.ActorAlign.CENTER}));
        card.add_child(new St.Label({style_class: 'amethystora-privacy-away-line', text: _('Unlock is always yours to do'),
            x_align: Clutter.ActorAlign.CENTER}));
        Main.uiGroup.add_child(card);
        told.cards++;
        const monitor = Main.layoutManager.primaryMonitor;
        if (monitor) {
            const [width, height] = card.get_preferred_size().slice(2);
            card.set_position(Math.round(monitor.x + (monitor.width - width) / 2),
                Math.round(monitor.y + (monitor.height - height) / 2));
        }
        card.connect('destroy', () => {
            if (this._card === card)
                this._card = null;
        });
    }

    // --- For the VM suite ---------------------------------------------------------------------------------------------

    state() {
        return {ready: this._ready, waiting: !!this._timer, watching: !!this._watch, card: !!this._card,
            locking: !!this._lockTimer, locked: told.locks, cardsShown: told.cards,
            listening: !!this._unlisten};
    }

    // A minute is long in a test: the suite passes the waits by calling these in turn
    goneForTest() {
        if (!this._timer)
            return false;
        GLib.source_remove(this._timer);
        this._timer = 0;
        this._gone();
        return true;
    }

    lockForTest() {
        this._lock();
    }
}
