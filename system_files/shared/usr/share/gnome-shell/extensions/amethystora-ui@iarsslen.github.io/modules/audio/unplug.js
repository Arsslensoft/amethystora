// Pause when the headphones come off: when the headphones or the headset in use are unplugged or go away, every player
// that is playing pauses, so that a podcast never bursts out of the laptop in a quiet room. A notice says so, with Play
// on speakers; when the headphones come back while it shows, it offers to play on.
//
// The shell's own sound control (the mixer GNOME's volume sliders use, no second connection to the sound server) says
// which output is in use and when one goes. Headphones are what GNOME's own output slider calls headphones: a form
// factor of headset or headphone, or a port whose name says headphone (ui/status/volume.js, _findHeadphones). Players
// are paused through their own media interface (MPRIS), on the session bus. A person picking the speakers while the
// headphones stay plugged in pauses nothing: only an output that went away does.
//
// Cost: three handlers on the mixer's signals; the bus is asked only at the moment the headphones go.

import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import Gvc from 'gi://Gvc';

import {gettext as _} from 'resource:///org/gnome/shell/extensions/extension.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import * as MessageTray from 'resource:///org/gnome/shell/ui/messageTray.js';
import * as Volume from 'resource:///org/gnome/shell/ui/status/volume.js';

const MPRIS_PREFIX = 'org.mpris.MediaPlayer2.';
const MPRIS_PATH = '/org/mpris/MediaPlayer2';
const PLAYER = 'org.mpris.MediaPlayer2.Player';
const CALL_TIMEOUT_MS = 2000;
// A headset that changes profile (for a call) is taken away and comes back as another output: the change is judged
// once the output that follows is known, or after this long
const SETTLE_MS = 300;

// The notice and the players it paused, kept across the lock (which turns the extension off and on again), so that
// Play on speakers still plays them after an unlock
let notice = null;
let pausedNames = [];

function call(name, path, iface, method, args, type, cancellable = null) {
    return Gio.DBus.session.call(name, path, iface, method, args, type ? new GLib.VariantType(type) : null,
        Gio.DBusCallFlags.NO_AUTO_START, CALL_TIMEOUT_MS, cancellable).then(reply => reply?.deepUnpack() ?? null);
}

// Play again what was paused, and only that
function playAgain() {
    const names = pausedNames;
    pausedNames = [];
    return Promise.all(names.map(name => call(name, MPRIS_PATH, PLAYER, 'Play', null, null).catch(() => null)));
}

// Headphones, by GNOME's own test of an output
function isHeadphones(mixer, device) {
    if (!device)
        return false;
    const port = device.get_port?.() ?? '';
    if (/headphone|headset/i.test(port))
        return true;
    const stream = mixer.get_stream_from_device(device);
    const form = stream?.get_form_factor?.() ?? '';
    return form === 'headset' || form === 'headphone';
}

export class Unplug {
    enable() {
        this._mixer = Volume.getMixerControl();
        this._cancellable = new Gio.Cancellable();
        this._active = null;
        this._lost = null;
        this._mixer.connectObject(
            'state-changed', () => this._remember(),
            'active-output-update', (mixer, id) => this._activeChanged(id),
            'output-removed', (mixer, id) => this._removed(id),
            this);
        this._remember();
    }

    disable() {
        this._cancellable?.cancel();
        this._cancellable = null;
        this._mixer?.disconnectObject(this);
        this._mixer = null;
        if (this._settleId)
            GLib.source_remove(this._settleId);
        this._settleId = 0;
        // Kept across the lock only
        if (!Main.sessionMode.isLocked) {
            notice?.destroy();
            notice = null;
            pausedNames = [];
        }
        this._active = null;
        this._lost = null;
    }

    // The output in use now, as {id, headphones}
    _current() {
        const sink = this._mixer?.get_default_sink();
        const device = sink ? this._mixer.lookup_device_from_stream(sink) : null;
        return device ? {id: device.get_id(), headphones: isHeadphones(this._mixer, device)} : null;
    }

    _remember() {
        if (this._mixer?.get_state() === Gvc.MixerControlState.READY)
            this._active = this._current();
    }

    _gone(id) {
        const device = this._mixer.lookup_output_id(id);
        return !device || device.port_available === false;
    }

    _activeChanged(id) {
        const before = this._active;
        const device = this._mixer.lookup_output_id(id);
        this._active = device ? {id, headphones: isHeadphones(this._mixer, device)} : this._current();
        // Headphones back while the notice shows: it offers to play on
        if (this._active?.headphones && pausedNames.length > 0)
            this._back();
        if (this._lost) {
            this._decide();
            return;
        }
        // The headphones went to another output: only when they are gone (a jack that is no longer plugged keeps its
        // output, unavailable), never when the person picked another
        if (before?.headphones && before.id !== id && this._gone(before.id))
            this._pause();
    }

    _removed(id) {
        if (!this._active?.headphones || this._active.id !== id)
            return;
        this._lost = this._active;
        this._active = null;
        if (this._settleId)
            GLib.source_remove(this._settleId);
        this._settleId = GLib.timeout_add(GLib.PRIORITY_DEFAULT, SETTLE_MS, () => {
            this._settleId = 0;
            this._active = this._current();
            this._decide();
            return GLib.SOURCE_REMOVE;
        });
    }

    // Headphones went: the players pause unless what follows is headphones too (a headset's other profile)
    _decide() {
        if (this._settleId)
            GLib.source_remove(this._settleId);
        this._settleId = 0;
        this._lost = null;
        if (!this._active?.headphones)
            this._pause();
    }

    // --- The players --------------------------------------------------------------------------------------------

    _call(name, path, iface, method, args, type) {
        return call(name, path, iface, method, args, type, this._cancellable);
    }

    // Every player that is playing now, by its bus name
    async _playing() {
        const [names] = await this._call('org.freedesktop.DBus', '/org/freedesktop/DBus', 'org.freedesktop.DBus',
            'ListNames', null, '(as)');
        const players = names.filter(name => name.startsWith(MPRIS_PREFIX));
        const states = await Promise.all(players.map(name => this._call(name, MPRIS_PATH,
            'org.freedesktop.DBus.Properties', 'Get', new GLib.Variant('(ss)', [PLAYER, 'PlaybackStatus']), '(v)')
            .then(([status]) => status.deepUnpack() === 'Playing')
            .catch(() => false)));
        return players.filter((name, i) => states[i]);
    }

    _pause() {
        const cancellable = this._cancellable;
        this._playing().then(players => Promise.all(players.map(name =>
            this._call(name, MPRIS_PATH, PLAYER, 'Pause', null, null).then(() => name).catch(() => null))))
            .then(paused => {
                if (cancellable.is_cancelled())
                    return;
                paused = paused.filter(Boolean);
                if (paused.length === 0)
                    return;
                pausedNames = paused;
                this._tell();
            }).catch(e => {
                if (!e.matches?.(Gio.IOErrorEnum, Gio.IOErrorEnum.CANCELLED))
                    console.warn(`Amethystora UI: the players could not be paused: ${e.message}`);
            });
    }

    play() {
        return playAgain();
    }

    // --- The notice ---------------------------------------------------------------------------------------------

    _tell() {
        notice?.destroy();
        const source = new MessageTray.Source({title: _('Sound'), iconName: 'audio-headphones-symbolic'});
        Main.messageTray.add(source);
        const notification = new MessageTray.Notification({
            source,
            title: _('Your headphones went away'),
            body: _('Music paused rather than playing out loud.'),
            isTransient: false,
        });
        notification.addAction(_('Play on speakers'), () => playAgain());
        notification.connect('destroy', () => {
            if (notice === notification) {
                notice = null;
                pausedNames = [];
            }
        });
        notice = notification;
        source.addNotification(notification);
    }

    _back() {
        if (!notice)
            return;
        notice.set({title: _('Your headphones are back'), body: _('Play what was paused?')});
        notice.clearActions();
        notice.addAction(_('Play'), () => playAgain());
    }

    // --- For the suites ---------------------------------------------------------------------------------------------

    state() {
        return {
            active: this._active,
            waiting: this._lost !== null,
            paused: [...pausedNames],
            notice: notice ? {title: notice.title, body: notice.body,
                actions: (notice.actions ?? []).map(a => a.label)} : null,
        };
    }

    // As if the headphones in use went away, without a device: the players playing now are paused and the notice shown
    pauseForTest() {
        this._pause();
        return true;
    }
}
