// The phone's players in the live pill (music from either side, plans/ui-ideas/spec-phone.md 3.2). The program lists a
// connected phone's players in its `Players` property while the person has the switch on; each one that has something to
// show becomes a stand-in here with the few readings and commands the pill's now playing takes of GNOME's own players
// (modules/pill/media.js), so that the phone's music is chosen and shown as the computer's own is. Nothing here reaches
// the session's MPRIS players: the titles stay between the program and the shell.

import GObject from 'gi://GObject';
import Shell from 'gi://Shell';

import {gettext as _} from 'resource:///org/gnome/shell/extensions/extension.js';

// The Phone app, whose icon the pill shows for a phone's player
const APP_ID = 'org.amethystora.Phone.desktop';

const same = (a, b) => Object.keys(a).every(key => a[key] === b[key]);

export const PhonePlayer = GObject.registerClass({
    Signals: {changed: {}},
}, class AmethystoraPhonePlayer extends GObject.Object {
    _init(device, entry) {
        super._init();
        this._device = device;
        this._entry = entry;
    }

    update(entry) {
        if (same(entry, this._entry))
            return;
        this._entry = entry;
        this.emit('changed');
    }

    get status() {
        return this._entry.playing ? 'Playing' : 'Paused';
    }

    get trackTitle() {
        return this._entry.title;
    }

    get trackArtists() {
        return this._entry.artist ? [this._entry.artist] : [];
    }

    // No picture comes from the phone
    get trackCoverUrl() {
        return '';
    }

    get app() {
        return Shell.AppSystem.get_default().lookup_app(APP_ID);
    }

    get canPlay() {
        return this._entry.canPlay || this._entry.canPause;
    }

    get canGoNext() {
        return this._entry.canNext;
    }

    get canGoPrevious() {
        return this._entry.canPrevious;
    }

    // Where it plays, for the pill's card: the player's name on the phone's
    get source() {
        // Translators: a phone's media player and the phone, in the live pill's card: "Spotify on Pixel 8"
        return _('%s on %s').format(this._entry.player, this._device.name);
    }

    // The track's length and where it was when the phone last said, in microseconds as MPRIS has them
    get trackLengthMicro() {
        return this._entry.length * 1000;
    }

    get trackPositionMicro() {
        return this._entry.position * 1000;
    }

    playPause() {
        this._act('PlayPause');
    }

    next() {
        this._act('Next');
    }

    previous() {
        this._act('Previous');
    }

    _act(action) {
        this._device.mediaAction(this._entry.player, action).catch(e => {
            console.debug(`Amethystora UI: the phone did not take ${action}: ${e.message}`);
        });
    }
});

// The stand-ins of every ready phone's players, with the two signals of GNOME's own list of players
export const PhonePlayers = GObject.registerClass({
    Signals: {
        'player-added': {param_types: [GObject.TYPE_OBJECT]},
        'player-removed': {param_types: [GObject.TYPE_OBJECT]},
    },
}, class AmethystoraPhonePlayers extends GObject.Object {
    _init() {
        super._init();
        this._players = new Map();
    }

    get players() {
        return [...this._players.values()];
    }

    // devices: the phones that are paired and connected now. A phone without the feature music (the switch off, or a
    // phone without players) has none here; a player with no title has nothing to show
    sync(devices) {
        const seen = new Set();
        for (const device of devices) {
            if (!device.features.has('music'))
                continue;
            for (const entry of device.players) {
                if (!entry.title)
                    continue;
                const key = `${device.path}\u0000${entry.player}`;
                seen.add(key);
                const known = this._players.get(key);
                if (known) {
                    known.update(entry);
                } else {
                    const player = new PhonePlayer(device, entry);
                    this._players.set(key, player);
                    this.emit('player-added', player);
                }
            }
        }
        for (const [key, player] of [...this._players]) {
            if (!seen.has(key)) {
                this._players.delete(key);
                this.emit('player-removed', player);
            }
        }
    }
});
